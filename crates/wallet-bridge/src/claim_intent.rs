//! Bind an encrypted funding memo to the client's bid/refund instructions.
//! Knowing a public funding cmx must not authorize claiming someone else's note.
//! This is client intent authentication, not chain validation or an input ZK proof.
use darkcall_auction_core::{MAX_MONEY, funding::Registration};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use orchard::{Note, note::NoteVersion};
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, String>;
const MEMO_DOMAIN: &[u8; 17] = b"DarkCall/fund/v1\0";
const CLAIM_DOMAIN: &[u8] = b"DarkCall/claim/v1\0";
const PAYLOAD_END: usize = 178;
const BN254_MODULUS: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00, 0x00, 0x01,
];

#[derive(Clone, PartialEq, Eq)]
pub struct DepositMemo {
    pub network_genesis: [u8; 32],
    pub conditions: [u8; 32],
    pub ticket: [u8; 32],
    /// A fresh application key per ticket, separate from native spending keys.
    pub client_key: [u8; 32],
    /// Fresh private random nonce. Never publish the memo or this nonce.
    pub nonce: [u8; 32],
}
fn valid_key(bytes: &[u8; 32]) -> Result<VerifyingKey> {
    let key = VerifyingKey::from_bytes(bytes).map_err(|_| "invalid client application key")?;
    if key.is_weak() {
        return Err("weak client application key".into());
    }
    Ok(key)
}
impl DepositMemo {
    pub fn encode(&self) -> Result<[u8; 512]> {
        valid_key(&self.client_key)?;
        let mut memo = [0; 512];
        memo[0] = 0xf5; // Standard arbitrary-data memo discriminator.
        memo[1..18].copy_from_slice(MEMO_DOMAIN);
        for (i, field) in [
            self.network_genesis,
            self.conditions,
            self.ticket,
            self.client_key,
            self.nonce,
        ]
        .iter()
        .enumerate()
        {
            memo[18 + i * 32..18 + (i + 1) * 32].copy_from_slice(field);
        }
        Ok(memo)
    }
    pub fn decode(bytes: &[u8; 512]) -> Result<Self> {
        if bytes[0] != 0xf5
            || &bytes[1..18] != MEMO_DOMAIN
            || bytes[PAYLOAD_END..].iter().any(|b| *b != 0)
        {
            return Err("unrecognized/noncanonical DarkCall funding memo".into());
        }
        let field = |i: usize| bytes[18 + i * 32..18 + (i + 1) * 32].try_into().unwrap();
        let memo = Self {
            network_genesis: field(0),
            conditions: field(1),
            ticket: field(2),
            client_key: field(3),
            nonce: field(4),
        };
        valid_key(&memo.client_key)?;
        Ok(memo)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instructions {
    pub bid_commitment: [u8; 32],
    pub pair_commitments: [[u8; 32]; 3],
    pub refund_receiver_commitment: [u8; 32],
}
impl Instructions {
    fn validate(&self) -> Result<()> {
        if [self.bid_commitment, self.refund_receiver_commitment]
            .iter()
            .chain(self.pair_commitments.iter())
            .any(|value| value >= &BN254_MODULUS)
        {
            return Err("noncanonical BN254 instruction commitment".into());
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Claim {
    memo_digest: [u8; 32],
    note_id: [u8; 32],
    instructions: Instructions,
    signature: Signature,
}
/// Private client-to-operator transport. Note association and memo digest must
/// stay private; signatures do not substitute for canonical chain admission.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireClaim {
    pub memo_digest: [u8; 32],
    pub note_id: [u8; 32],
    pub instructions: Instructions,
    pub signature: Vec<u8>,
}
impl Claim {
    pub fn to_wire(&self) -> WireClaim {
        WireClaim {
            memo_digest: self.memo_digest,
            note_id: self.note_id,
            instructions: self.instructions.clone(),
            signature: self.signature.to_bytes().to_vec(),
        }
    }
    pub fn from_wire(wire: WireClaim) -> Result<Self> {
        wire.instructions.validate()?;
        let signature =
            Signature::from_slice(&wire.signature).map_err(|_| "invalid claim signature length")?;
        Ok(Self {
            memo_digest: wire.memo_digest,
            note_id: wire.note_id,
            instructions: wire.instructions,
            signature,
        })
    }
    fn message(&self) -> Result<Vec<u8>> {
        self.instructions.validate()?;
        let mut bytes = CLAIM_DOMAIN.to_vec();
        bytes.extend(self.memo_digest);
        bytes.extend(self.note_id);
        bytes.extend(self.instructions.bid_commitment);
        for value in self.instructions.pair_commitments {
            bytes.extend(value);
        }
        bytes.extend(self.instructions.refund_receiver_commitment);
        Ok(bytes)
    }
    /// Sign after the sender wallet knows the actual output cmx. Memo/key/nonce
    /// must be fixed before deposit construction; no native spend key is needed.
    pub fn sign(
        memo: &DepositMemo,
        note_id: [u8; 32],
        instructions: Instructions,
        key: &SigningKey,
    ) -> Result<Self> {
        if key.verifying_key().to_bytes() != memo.client_key {
            return Err("claim key differs from the encrypted funding memo".into());
        }
        let mut claim = Self {
            memo_digest: Sha256::digest(memo.encode()?).into(),
            note_id,
            instructions,
            signature: Signature::from_bytes(&[0; 64]),
        };
        claim.signature = key.sign(&claim.message()?);
        Ok(claim)
    }
}

pub struct ExpectedFunding {
    pub network_genesis: [u8; 32],
    pub conditions: [u8; 32],
    pub ticket: [u8; 32],
    pub required_value: u64,
}
/// Created only after memo context, amount, actual cmx and client signature pass.
/// Client ZK/share validity, canonical funding and deadline still need checking.
pub struct VerifiedClaim {
    registration: Registration,
    pair_commitments: [[u8; 32]; 3],
    pub(crate) network_genesis: [u8; 32],
    pub(crate) conditions: [u8; 32],
    pub(crate) value: u64,
    pub(crate) intent_digest: [u8; 32],
}
impl VerifiedClaim {
    pub fn registration(&self) -> &Registration {
        &self.registration
    }
    pub fn pair_commitments(&self) -> &[[u8; 32]; 3] {
        &self.pair_commitments
    }
}
/// Authenticate client instructions against a natively decrypted V3 note/memo.
/// Cross-version adapters must decrypt the actual ciphertext in their pinned
/// SDK and compare the reconstructed commitment bytes before calling this.
/// This checks intent only: the caller still supplies trusted canonical chain,
/// unspent, deadline and client-proof/share validation. Plain note openings alone
/// are not evidence of funding, and this function does not decrypt ciphertext.
pub fn verify_decrypted_note(
    note: &Note,
    memo_bytes: &[u8; 512],
    expected: &ExpectedFunding,
    claim: &Claim,
) -> Result<VerifiedClaim> {
    let memo = DepositMemo::decode(memo_bytes)?;
    if memo.network_genesis != expected.network_genesis
        || memo.conditions != expected.conditions
        || memo.ticket != expected.ticket
    {
        return Err("funding memo belongs to another network/auction/ticket".into());
    }
    if expected.required_value > MAX_MONEY
        || note.version() != NoteVersion::V3
        || note.value().inner() != expected.required_value
    {
        return Err("funding note has the wrong version/amount".into());
    }
    let id = orchard::note::ExtractedNoteCommitment::from(note.commitment()).to_bytes();
    if id != claim.note_id || <[u8; 32]>::from(Sha256::digest(memo_bytes)) != claim.memo_digest {
        return Err("claim does not match the actual funding output/memo".into());
    }
    valid_key(&memo.client_key)?
        .verify_strict(&claim.message()?, &claim.signature)
        .map_err(|_| "funding claim signature invalid")?;
    Ok(VerifiedClaim {
        registration: Registration {
            ticket: memo.ticket,
            note: id,
            bid_commitment: claim.instructions.bid_commitment,
            receiver_commitment: claim.instructions.refund_receiver_commitment,
        },
        pair_commitments: claim.instructions.pair_commitments,
        network_genesis: memo.network_genesis,
        conditions: memo.conditions,
        value: note.value().inner(),
        intent_digest: Sha256::digest(
            [
                b"DarkCall/intent/v1\0".as_slice(),
                &claim.message()?,
                &claim.signature.to_bytes(),
            ]
            .concat(),
        )
        .into(),
    })
}

#[cfg(test)]
pub(crate) fn synthetic_verified_claim() -> VerifiedClaim {
    let (note, memo, key, instructions, expected) = tests::fixture();
    let id = orchard::note::ExtractedNoteCommitment::from(note.commitment()).to_bytes();
    let claim = Claim::sign(&memo, id, instructions, &key).unwrap();
    verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &claim).unwrap()
}

#[cfg(test)]
pub(crate) fn synthetic_verified_claim_for(
    ticket: [u8; 32],
    instructions: Instructions,
) -> VerifiedClaim {
    let (note, mut memo, key, _, mut expected) = tests::fixture();
    memo.ticket = ticket;
    expected.ticket = ticket;
    let id = orchard::note::ExtractedNoteCommitment::from(note.commitment()).to_bytes();
    let claim = Claim::sign(&memo, id, instructions, &key).unwrap();
    verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &claim).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchard::{
        keys::{FullViewingKey, Scope, SpendingKey},
        note::{RandomSeed, Rho},
        value::NoteValue,
    };
    use rand::{RngCore, rngs::OsRng};
    pub(super) fn fixture() -> (Note, DepositMemo, SigningKey, Instructions, ExpectedFunding) {
        let escrow = FullViewingKey::from(&SpendingKey::from_bytes([1; 32]).unwrap());
        let key = SigningKey::from_bytes(&[7; 32]);
        let memo = DepositMemo {
            network_genesis: [8; 32],
            conditions: [9; 32],
            ticket: [10; 32],
            client_key: key.verifying_key().to_bytes(),
            nonce: [11; 32],
        };
        let rho = Rho::from_bytes(&[1; 32]).unwrap();
        let seed = loop {
            let mut bytes = [0; 32];
            OsRng.fill_bytes(&mut bytes);
            if let Some(seed) = RandomSeed::from_bytes(bytes, &rho).into_option() {
                break seed;
            }
        };
        let note = Note::from_parts(
            escrow.address_at(0u32, Scope::External),
            NoteValue::from_raw(15100),
            rho,
            seed,
            NoteVersion::V3,
        )
        .unwrap();
        let instructions = Instructions {
            bid_commitment: [3; 32],
            pair_commitments: [[4; 32]; 3],
            refund_receiver_commitment: [5; 32],
        };
        let expected = ExpectedFunding {
            network_genesis: memo.network_genesis,
            conditions: memo.conditions,
            ticket: memo.ticket,
            required_value: 15100,
        };
        (note, memo, key, instructions, expected)
    }
    #[test]
    fn public_note_id_does_not_authorize_refund_or_bid_substitution() {
        let (note, memo, key, instructions, expected) = fixture();
        let id = orchard::note::ExtractedNoteCommitment::from(note.commitment()).to_bytes();
        let claim = Claim::sign(&memo, id, instructions, &key).unwrap();
        let verified =
            verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &claim).unwrap();
        assert_eq!(verified.registration().note, id);
        let mut changed = claim.clone();
        changed.instructions.refund_receiver_commitment[0] ^= 1;
        assert!(
            verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &changed).is_err()
        );
        let mut changed = claim.clone();
        changed.instructions.pair_commitments[2][0] ^= 1;
        assert!(
            verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &changed).is_err()
        );
        let mut changed = claim.clone();
        changed.instructions.bid_commitment[0] ^= 1;
        assert!(
            verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &changed).is_err()
        );
        let mut changed = claim.clone();
        changed.note_id[0] ^= 1;
        assert!(
            verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &changed).is_err()
        );
        assert!(
            Claim::sign(
                &memo,
                id,
                claim.instructions.clone(),
                &SigningKey::from_bytes(&[12; 32])
            )
            .is_err()
        );
    }
    #[test]
    fn private_wire_roundtrip_rejects_substitution_and_malformed_signature() {
        let (note, memo, key, instructions, expected) = fixture();
        let id = orchard::note::ExtractedNoteCommitment::from(note.commitment()).to_bytes();
        let claim = Claim::sign(&memo, id, instructions, &key).unwrap();
        let wire = claim.to_wire();
        let decoded =
            Claim::from_wire(serde_json::from_slice(&serde_json::to_vec(&wire).unwrap()).unwrap())
                .unwrap();
        assert!(verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &decoded).is_ok());
        let mut changed = wire.clone();
        changed.instructions.refund_receiver_commitment[31] ^= 1;
        assert!(
            verify_decrypted_note(
                &note,
                &memo.encode().unwrap(),
                &expected,
                &Claim::from_wire(changed).unwrap()
            )
            .is_err()
        );
        let mut changed = wire.clone();
        changed.signature.pop();
        assert!(Claim::from_wire(changed).is_err());
        let mut changed = wire;
        changed.instructions.bid_commitment = BN254_MODULUS;
        assert!(Claim::from_wire(changed).is_err());
    }
    #[test]
    fn memo_context_alias_amount_and_weak_key_are_rejected() {
        let (note, memo, key, instructions, mut expected) = fixture();
        let id = orchard::note::ExtractedNoteCommitment::from(note.commitment()).to_bytes();
        let claim = Claim::sign(&memo, id, instructions, &key).unwrap();
        let mut bytes = memo.encode().unwrap();
        bytes[511] = 1;
        assert!(DepositMemo::decode(&bytes).is_err());
        expected.network_genesis[0] ^= 1;
        assert!(verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &claim).is_err());
        expected.network_genesis[0] ^= 1;
        expected.required_value = 15101;
        assert!(verify_decrypted_note(&note, &memo.encode().unwrap(), &expected, &claim).is_err());
        let mut alias = claim.clone();
        alias.instructions.bid_commitment = BN254_MODULUS;
        assert!(alias.message().is_err());
        let mut weak = memo;
        weak.client_key = [0; 32];
        assert!(weak.encode().is_err());
    }
}
