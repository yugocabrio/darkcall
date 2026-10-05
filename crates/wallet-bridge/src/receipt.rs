//! Application acceptance receipts, separate from native FROST authorization.
//! Receipt issuance attests what an operator accepted, not a public ZK proof of
//! funding. Configure operator identities from trusted auction setup.
use crate::{
    claim::VerifiedClaim,
    journal::{Context, Event, Journal},
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, String>;
const DOMAIN: &[u8] = b"DarkCall/accepted-receipt/v1\0";
/// All three distinct application keys endorse one payload. Never accept this
/// identity list from an untrusted receipt. It belongs in auction configuration.
pub struct Operators {
    keys: [VerifyingKey; 3],
}
impl Operators {
    pub fn new(bytes: [[u8; 32]; 3]) -> Result<Self> {
        if bytes[0] == bytes[1] || bytes[1] == bytes[2] || bytes[0] == bytes[2] {
            return Err("operator application keys must be distinct".into());
        }
        let mut keys = Vec::new();
        for bytes in bytes {
            let key =
                VerifyingKey::from_bytes(&bytes).map_err(|_| "invalid operator application key")?;
            if key.is_weak() {
                return Err("weak operator application key".into());
            }
            keys.push(key);
        }
        Ok(Self {
            keys: keys.try_into().map_err(|_| "operator count")?,
        })
    }
    fn context_hash(&self, context: &Context) -> Result<[u8; 32]> {
        if context.format != 1 || context.hash_rule.len() > 128 || context.hash_rule.is_empty() {
            return Err("unsupported receipt context".into());
        }
        let mut bytes = b"DarkCall/receipt-context/v1\0".to_vec();
        bytes.extend(context.format.to_le_bytes());
        bytes.extend(context.network_genesis);
        bytes.extend(context.conditions_hash);
        bytes.extend((context.hash_rule.len() as u16).to_le_bytes());
        bytes.extend(context.hash_rule.as_bytes());
        bytes.extend(context.units.to_le_bytes());
        for value in [context.reserve, context.budget, context.processing_fee] {
            bytes.extend(value.to_le_bytes());
        }
        for key in self.keys {
            bytes.extend(key.to_bytes());
        }
        Ok(Sha256::digest(bytes).into())
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Payload {
    pub context_hash: [u8; 32],
    pub ticket: [u8; 32],
    pub bid_commitment: [u8; 32],
    pub pair_commitments: [[u8; 32]; 3],
    pub refund_receiver_commitment: [u8; 32],
    /// Opaque authenticated claim digest. No cmx, nonce, viewing key, actual
    /// amount or recipient opening is published in this payload.
    pub intent_digest: [u8; 32],
}
impl Payload {
    fn message(&self) -> Vec<u8> {
        let mut bytes = DOMAIN.to_vec();
        bytes.extend(self.context_hash);
        bytes.extend(self.ticket);
        bytes.extend(self.bid_commitment);
        for value in self.pair_commitments {
            bytes.extend(value);
        }
        bytes.extend(self.refund_receiver_commitment);
        bytes.extend(self.intent_digest);
        bytes
    }
}
#[derive(Clone)]
pub struct Receipt {
    payload: Payload,
    operator: usize,
    signature: Signature,
}
impl Receipt {
    /// Issue after chain confirmation, client ZK and this node's delivered-pair
    /// checks caused the exact registration to be accepted in the journal.
    /// A client intent signature by itself is insufficient admission.
    pub fn issue(
        journal: &mut Journal,
        claim: &VerifiedClaim,
        operators: &Operators,
        operator: usize,
        key: &SigningKey,
    ) -> Result<Self> {
        if operator >= 3 || operators.keys[operator] != key.verifying_key() {
            return Err("receipt signer is not the configured operator".into());
        }
        let context = journal.context();
        if claim.network_genesis != context.network_genesis
            || claim.conditions != context.conditions_hash
            || Some(claim.value) != context.budget.checked_add(context.processing_fee)
        {
            return Err("claim differs from journal network/conditions/deposit amount".into());
        }
        let requested = claim.registration();
        let accepted = journal
            .registration(&requested.ticket)
            .ok_or("ticket not accepted")?;
        if accepted.note != requested.note
            || accepted.bid_commitment != requested.bid_commitment
            || accepted.receiver_commitment != requested.receiver_commitment
        {
            return Err("authenticated claim differs from accepted funding registration".into());
        }
        let payload = Payload {
            context_hash: operators.context_hash(context)?,
            ticket: requested.ticket,
            bid_commitment: requested.bid_commitment,
            pair_commitments: *claim.pair_commitments(),
            refund_receiver_commitment: requested.receiver_commitment,
            intent_digest: claim.intent_digest,
        };
        let bytes = payload.message();
        let digest = Sha256::digest(&bytes).into();
        // Durable before signing. Restart can reissue an identical deterministic
        // signature but cannot endorse different accepted instructions.
        journal.append(Event::LockReceipt {
            ticket: payload.ticket,
            digest,
        })?;
        Ok(Self {
            payload,
            operator,
            signature: key.sign(&bytes),
        })
    }
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
    pub fn verify(&self, operators: &Operators, expected: &Context) -> Result<()> {
        if self.operator >= 3 || self.payload.context_hash != operators.context_hash(expected)? {
            return Err("receipt authority/context mismatch".into());
        }
        operators.keys[self.operator]
            .verify_strict(&self.payload.message(), &self.signature)
            .map_err(|_| "operator receipt signature invalid".into())
    }
}
/// Three matching endorsements. Complete deadline-book/root certification and
/// omission detection against the actual MPC statement remain a separate step.
pub struct AcceptedReceipt {
    receipts: [Receipt; 3],
}
impl AcceptedReceipt {
    pub fn combine(
        mut receipts: [Receipt; 3],
        operators: &Operators,
        context: &Context,
    ) -> Result<Self> {
        receipts.sort_by_key(|r| r.operator);
        for (operator, receipt) in receipts.iter().enumerate() {
            receipt.verify(operators, context)?;
            if receipt.operator != operator || receipt.payload != receipts[0].payload {
                return Err(
                    "receipt quorum missing an operator or contains different instructions".into(),
                );
            }
        }
        Ok(Self { receipts })
    }
    pub fn payload(&self) -> &Payload {
        &self.receipts[0].payload
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim::synthetic_verified_claim;
    struct Fixture {
        directory: std::path::PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "darkcall-receipt-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&directory).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                    .unwrap();
            }
            Self { directory }
        }
        fn journal(&self, operator: usize) -> std::path::PathBuf {
            self.directory.join(format!("operator-{operator}.jsonl"))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.directory).unwrap();
        }
    }
    fn context() -> Context {
        Context {
            format: 1,
            network_genesis: [8; 32],
            conditions_hash: [9; 32],
            hash_rule: "bn254-poseidon2-packed-row-v1".into(),
            units: 3,
            reserve: 20,
            budget: 100,
            processing_fee: 15000,
        }
    }
    fn accept(journal: &mut Journal, claim: &VerifiedClaim) {
        let reg = claim.registration();
        journal
            .append(Event::Observe {
                note: reg.note,
                value: 15100,
                block: [1; 32],
                height: 10,
            })
            .unwrap();
        journal
            .append(Event::Confirm {
                note: reg.note,
                block: [1; 32],
            })
            .unwrap();
        journal
            .append(Event::Accept {
                ticket: reg.ticket,
                note: reg.note,
                bid_commitment: reg.bid_commitment,
                receiver_commitment: reg.receiver_commitment,
            })
            .unwrap();
    }
    #[test]
    fn matching_quorum_and_restart_cannot_change_receipt() {
        let f = Fixture::new();
        let claim = synthetic_verified_claim();
        let keys = [1u8, 2, 3].map(|n| SigningKey::from_bytes(&[n; 32]));
        let operators =
            Operators::new(keys.each_ref().map(|key| key.verifying_key().to_bytes())).unwrap();
        let receipts = std::array::from_fn(|operator| {
            let mut journal = Journal::open(&f.journal(operator), &context()).unwrap();
            accept(&mut journal, &claim);
            Receipt::issue(&mut journal, &claim, &operators, operator, &keys[operator]).unwrap()
        });
        let accepted = AcceptedReceipt::combine(receipts.clone(), &operators, &context()).unwrap();
        assert_eq!(accepted.payload().ticket, claim.registration().ticket);
        let mut journal = Journal::open(&f.journal(0), &context()).unwrap();
        let repeated = Receipt::issue(&mut journal, &claim, &operators, 0, &keys[0]).unwrap();
        repeated.verify(&operators, &context()).unwrap();
        assert_eq!(repeated.signature, receipts[0].signature);
        assert!(
            journal
                .append(Event::LockReceipt {
                    ticket: claim.registration().ticket,
                    digest: [0; 32]
                })
                .is_err()
        );
        let mut forged = receipts[0].clone();
        forged.payload.refund_receiver_commitment[0] ^= 1;
        assert!(forged.verify(&operators, &context()).is_err());
        assert!(
            AcceptedReceipt::combine(
                [
                    receipts[0].clone(),
                    receipts[0].clone(),
                    receipts[2].clone()
                ],
                &operators,
                &context()
            )
            .is_err()
        );
        let mut wrong = context();
        wrong.conditions_hash[0] ^= 1;
        assert!(receipts[0].verify(&operators, &wrong).is_err());
    }
    #[test]
    fn accepted_funding_and_configured_authority_are_required() {
        let f = Fixture::new();
        let claim = synthetic_verified_claim();
        let keys = [1u8, 2, 3].map(|n| SigningKey::from_bytes(&[n; 32]));
        let bytes = keys.each_ref().map(|key| key.verifying_key().to_bytes());
        assert!(Operators::new([bytes[0]; 3]).is_err());
        let operators = Operators::new(bytes).unwrap();
        let mut journal = Journal::open(&f.journal(0), &context()).unwrap();
        assert!(Receipt::issue(&mut journal, &claim, &operators, 0, &keys[0]).is_err());
        accept(&mut journal, &claim);
        assert!(Receipt::issue(&mut journal, &claim, &operators, 0, &keys[1]).is_err());
        journal.append(Event::Freeze).unwrap();
        journal
            .append(Event::Invalidate {
                notes: std::collections::BTreeSet::from([claim.registration().note]),
            })
            .unwrap();
        assert!(Receipt::issue(&mut journal, &claim, &operators, 0, &keys[0]).is_err());
    }
}
