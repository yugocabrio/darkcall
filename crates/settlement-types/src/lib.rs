//! Canonical row encoding and public statement linkage, not proof verification.
//! A caller must separately verify auction/native proofs and obtain transaction
//! commitments from its trusted node before using the structural link checker.
use sha2::{Digest, Sha256};

pub const ROW_BYTES: usize = 224;
pub const ROW_DOMAIN: &[u8; 16] = b"DarkCall/row/v1\0";
pub const FOUR_BID_OUTPUTS: usize = 6;

/// Public synthetic context transport. This is not an acceptance/funding receipt.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeContext {
    pub conditions: [u8; 32],
    pub accepted_root: [u8; 32],
    pub ticket_ids: [[u8; 32]; 4],
}

/// Public inputs to the fixed six-row native proof, plus the output-only native
/// fixture's action commitments. It proves neither chain inclusion nor funding.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBatchStatement {
    pub row_hashes: [[u32; 8]; 6],
    pub action_indices: [u32; 6],
    pub cmxs: [[u8; 32]; 6],
    pub native_actions: [[u8; 32]; 6],
}
impl NativeBatchStatement {
    pub fn bindings(&self) -> [BindingStatement; 6] {
        std::array::from_fn(|i| BindingStatement {
            row_hash: self.row_hashes[i],
            cmx: self.cmxs[i],
            action_index: self.action_indices[i],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Role {
    Refund = 0,
    Seller = 1,
    Operator = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    WrongLength,
    WrongDomain,
    UnknownRole,
}

/// Private opening. Intentionally does not implement Debug.
#[derive(Clone)]
pub struct Row {
    pub conditions: [u8; 32],
    pub accepted_root: [u8; 32],
    pub ticket: [u8; 32],
    pub role: Role,
    pub action_index: u32,
    pub gd: [u8; 32],
    pub pkd: [u8; 32],
    pub amount: u64,
    pub salt: [u8; 32],
}
impl Row {
    /// Decode the fixed wire format. Point/field validity, authenticated context
    /// and proof validity are deliberately checked by the caller, not this codec.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() != ROW_BYTES {
            return Err(DecodeError::WrongLength);
        }
        if &bytes[..16] != ROW_DOMAIN {
            return Err(DecodeError::WrongDomain);
        }
        let role = match u32::from_le_bytes(bytes[112..116].try_into().unwrap()) {
            0 => Role::Refund,
            1 => Role::Seller,
            2 => Role::Operator,
            _ => return Err(DecodeError::UnknownRole),
        };
        Ok(Self {
            conditions: bytes[16..48].try_into().unwrap(),
            accepted_root: bytes[48..80].try_into().unwrap(),
            ticket: bytes[80..112].try_into().unwrap(),
            role,
            action_index: u32::from_le_bytes(bytes[116..120].try_into().unwrap()),
            gd: bytes[120..152].try_into().unwrap(),
            pkd: bytes[152..184].try_into().unwrap(),
            amount: u64::from_le_bytes(bytes[184..192].try_into().unwrap()),
            salt: bytes[192..224].try_into().unwrap(),
        })
    }
    pub fn encode(&self) -> [u8; ROW_BYTES] {
        let mut bytes = [0; ROW_BYTES];
        bytes[..16].copy_from_slice(ROW_DOMAIN);
        bytes[16..48].copy_from_slice(&self.conditions);
        bytes[48..80].copy_from_slice(&self.accepted_root);
        bytes[80..112].copy_from_slice(&self.ticket);
        bytes[112..116].copy_from_slice(&(self.role as u32).to_le_bytes());
        bytes[116..120].copy_from_slice(&self.action_index.to_le_bytes());
        bytes[120..152].copy_from_slice(&self.gd);
        bytes[152..184].copy_from_slice(&self.pkd);
        bytes[184..192].copy_from_slice(&self.amount.to_le_bytes());
        bytes[192..224].copy_from_slice(&self.salt);
        bytes
    }

    pub fn commitment(&self) -> [u32; 8] {
        let hash = Sha256::digest(self.encode());
        std::array::from_fn(|i| u32::from_be_bytes(hash[i * 4..i * 4 + 4].try_into().unwrap()))
    }
}

/// Public native-proof inputs. This type does not assert the proof was verified.
#[derive(Clone, Copy, Debug)]
pub struct BindingStatement {
    pub row_hash: [u32; 8],
    pub cmx: [u8; 32],
    pub action_index: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkError {
    WrongActionCount,
    DifferentRow,
    IndexOutsideShape,
    ReusedAction,
    DifferentNativeCommitment,
}

/// Check the correspondence between already verified statements and actual
/// transaction commitments, in canonical refund/seller/operator row order.
/// This establishes neither proof validity, chain inclusion nor funding.
pub fn link_four_bid_statements(
    auction_rows: &[[u32; 8]; FOUR_BID_OUTPUTS],
    bindings: &[BindingStatement; FOUR_BID_OUTPUTS],
    native_actions: &[[u8; 32]],
) -> Result<(), LinkError> {
    if native_actions.len() != FOUR_BID_OUTPUTS {
        return Err(LinkError::WrongActionCount);
    }
    let mut used = [false; FOUR_BID_OUTPUTS];
    for (expected, binding) in auction_rows.iter().zip(bindings) {
        if *expected != binding.row_hash {
            return Err(LinkError::DifferentRow);
        }
        let index =
            usize::try_from(binding.action_index).map_err(|_| LinkError::IndexOutsideShape)?;
        if index >= FOUR_BID_OUTPUTS {
            return Err(LinkError::IndexOutsideShape);
        }
        if used[index] {
            return Err(LinkError::ReusedAction);
        }
        used[index] = true;
        if native_actions[index] != binding.cmx {
            return Err(LinkError::DifferentNativeCommitment);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn row_matches_python_and_noir_vector() {
        let mut root = [0; 32];
        root[31] = 2;
        let row = Row {
            conditions: [1; 32],
            accepted_root: root,
            ticket: [3; 32],
            role: Role::Refund,
            action_index: 5,
            gd: [4; 32],
            pkd: [5; 32],
            amount: 50,
            salt: [6; 32],
        };
        assert_eq!(
            row.commitment(),
            [
                2976452346, 2644444602, 4093154051, 1752889704, 1917315153, 1897124546, 1306150620,
                994172045
            ]
        );
        let encoded = row.encode();
        assert_eq!(Row::decode(&encoded).unwrap().encode(), encoded);
        assert!(matches!(
            Row::decode(&encoded[..223]),
            Err(DecodeError::WrongLength)
        ));
        let mut bad = encoded;
        bad[0] ^= 1;
        assert!(matches!(Row::decode(&bad), Err(DecodeError::WrongDomain)));
        let mut bad = encoded;
        bad[112] = 3;
        assert!(matches!(Row::decode(&bad), Err(DecodeError::UnknownRole)));
    }
    #[test]
    fn native_statement_mapping_rejects_row_transaction_and_action_substitution() {
        let hashes = std::array::from_fn(|i| [i as u32; 8]);
        let indices = [5, 2, 0, 4, 1, 3];
        let actions: Vec<_> = (0..6).map(|i| [i as u8; 32]).collect();
        let bindings: [BindingStatement; 6] = std::array::from_fn(|i| BindingStatement {
            row_hash: hashes[i],
            cmx: actions[indices[i]],
            action_index: indices[i] as u32,
        });
        assert_eq!(
            link_four_bid_statements(&hashes, &bindings, &actions),
            Ok(())
        );
        let mut bad = bindings;
        bad[0].row_hash[0] ^= 1;
        assert_eq!(
            link_four_bid_statements(&hashes, &bad, &actions),
            Err(LinkError::DifferentRow)
        );
        let mut other_tx = actions.clone();
        other_tx.swap(0, 5);
        assert_eq!(
            link_four_bid_statements(&hashes, &bindings, &other_tx),
            Err(LinkError::DifferentNativeCommitment)
        );
        bad = bindings;
        bad[1].action_index = bad[0].action_index;
        assert_eq!(
            link_four_bid_statements(&hashes, &bad, &actions),
            Err(LinkError::ReusedAction)
        );
        assert_eq!(
            link_four_bid_statements(&hashes, &bindings, &actions[..5]),
            Err(LinkError::WrongActionCount)
        );
        bad = bindings;
        bad[0].action_index = 6;
        assert_eq!(
            link_four_bid_statements(&hashes, &bad, &actions),
            Err(LinkError::IndexOutsideShape)
        );
    }
}
