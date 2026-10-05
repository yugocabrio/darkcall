//! All-three final-book authentication and linkage to actual Noir public bytes.
//! Signatures attest operator admission; they are not public funding proofs.
use crate::{
    journal::{Context, Event, Journal},
    receipt::{AcceptedReceipt, Operators, Payload},
};
use darkcall_auction_core::{Terms, clear, funding::Phase};
use darkcall_input_tool::{canonical_field, four_active_root};
use ed25519_dalek::{Signature, Signer, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, String>;
pub const HASH_RULE: &str = "bn254-poseidon2-packed-row-v1";
const DOMAIN: &[u8] = b"DarkCall/final-book/v1\0";

/// Trusted out-of-band auction setup. Never load an authority manifest from the
/// same untrusted bundle as the certificate without authenticating it first.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub context: Context,
    pub operator_keys: [[u8; 32]; 3],
    pub closes_at_utc: u64,
    pub seller_receiver_commitment: [u8; 32],
    pub operator_receiver_commitment: [u8; 32],
}
impl Manifest {
    pub fn operators(&self) -> Result<Operators> {
        if self.context.hash_rule != HASH_RULE || self.closes_at_utc == 0 {
            return Err("unsupported final-book commitment rule/cutoff".into());
        }
        clear(
            Terms {
                units: self.context.units,
                reserve: self.context.reserve,
                budget: self.context.budget,
                processing_fee: self.context.processing_fee,
            },
            &[],
        )
        .map_err(|_| "invalid final-book terms")?;
        let deposit = self
            .context
            .budget
            .checked_add(self.context.processing_fee)
            .ok_or("four-ticket deposit overflow")?;
        if deposit
            .checked_mul(4)
            .is_none_or(|value| value > darkcall_auction_core::MAX_MONEY)
            || self
                .context
                .processing_fee
                .checked_mul(4)
                .is_none_or(|value| value < 30_000)
        {
            return Err(
                "four-ticket terms exceed native money bound or cannot pay standard fee".into(),
            );
        }
        canonical_field(&self.seller_receiver_commitment)?;
        canonical_field(&self.operator_receiver_commitment)?;
        Operators::new(self.operator_keys)
    }
    fn digest(&self) -> Result<[u8; 32]> {
        let operators = self.operators()?;
        let mut bytes = b"DarkCall/final-manifest/v1\0".to_vec();
        bytes.extend(operators.context_hash(&self.context)?);
        bytes.extend(self.closes_at_utc.to_le_bytes());
        bytes.extend(self.seller_receiver_commitment);
        bytes.extend(self.operator_receiver_commitment);
        Ok(Sha256::digest(bytes).into())
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalBook {
    pub manifest_hash: [u8; 32],
    /// Canonical unsigned lexicographic ticket order. This prototype supports
    /// exactly four active accepted tickets, not padded or 32-ticket books.
    pub entries: [Payload; 4],
    pub accepted_root: [u8; 32],
}
impl FinalBook {
    pub fn from_receipts(manifest: &Manifest, receipts: &[AcceptedReceipt; 4]) -> Result<Self> {
        let mut entries = receipts.each_ref().map(|r| r.payload().clone());
        entries.sort_by_key(|entry| entry.ticket);
        let c = &manifest.context;
        let accepted_root = four_active_root(
            &c.conditions_hash,
            c.units,
            c.reserve,
            c.budget,
            c.processing_fee,
            entries.each_ref().map(|p| p.bid_commitment),
        )?;
        let book = Self {
            manifest_hash: manifest.digest()?,
            entries,
            accepted_root,
        };
        book.validate(manifest)?;
        Ok(book)
    }
    fn validate(&self, manifest: &Manifest) -> Result<()> {
        let operators = manifest.operators()?;
        if self.manifest_hash != manifest.digest()? {
            return Err("final book differs from trusted auction manifest".into());
        }
        let expected = operators.context_hash(&manifest.context)?;
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.context_hash != expected
                || (i > 0 && self.entries[i - 1].ticket >= entry.ticket)
            {
                return Err("foreign, duplicate or noncanonical final-book tickets".into());
            }
            canonical_field(&entry.bid_commitment)?;
            canonical_field(&entry.refund_receiver_commitment)?;
            for pair in entry.pair_commitments {
                canonical_field(&pair)?;
            }
        }
        let c = &manifest.context;
        if four_active_root(
            &c.conditions_hash,
            c.units,
            c.reserve,
            c.budget,
            c.processing_fee,
            self.entries.each_ref().map(|p| p.bid_commitment),
        )? != self.accepted_root
        {
            return Err("final accepted root differs from exact Noir encoding".into());
        }
        Ok(())
    }
    fn message(&self) -> Vec<u8> {
        let mut bytes = DOMAIN.to_vec();
        bytes.extend(self.manifest_hash);
        bytes.extend(4u32.to_le_bytes());
        for entry in &self.entries {
            bytes.extend(entry.message());
        }
        bytes.extend(self.accepted_root);
        bytes
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endorsement {
    pub book: FinalBook,
    pub operator: usize,
    pub signature: String,
}
impl Endorsement {
    /// The caller must have enforced the declared cutoff during admission and
    /// checked funding/client proofs. now_utc is its trusted clock observation,
    /// not a cryptographic proof of time. Reorg reconciliation must precede freeze.
    pub fn issue(
        journal: &mut Journal,
        book: &FinalBook,
        manifest: &Manifest,
        operator: usize,
        key: &SigningKey,
        now_utc: u64,
    ) -> Result<Self> {
        book.validate(manifest)?;
        let operators = manifest.operators()?;
        if operator >= 3
            || operators.keys[operator] != key.verifying_key()
            || *journal.context() != manifest.context
            || now_utc < manifest.closes_at_utc
            || journal.phase() != Phase::Frozen
        {
            return Err("final-book signer/context/cutoff/funding phase mismatch".into());
        }
        let registrations: Vec<_> = journal.registrations().collect();
        if registrations.len() != 4 {
            return Err("final book omits or adds accepted funding".into());
        }
        for (registration, payload) in registrations.into_iter().zip(&book.entries) {
            if registration.ticket != payload.ticket
                || registration.bid_commitment != payload.bid_commitment
                || registration.receiver_commitment != payload.refund_receiver_commitment
            {
                return Err("final book differs from complete local accepted funding".into());
            }
            let receipt_digest: [u8; 32] = Sha256::digest(payload.message()).into();
            if journal.receipt_lock(&payload.ticket) != Some(&receipt_digest) {
                return Err("final book differs from locally locked accepted receipt".into());
            }
        }
        let message = book.message();
        journal.append(Event::LockFinalBook {
            digest: Sha256::digest(&message).into(),
        })?;
        Ok(Self {
            book: book.clone(),
            operator,
            signature: hex::encode(key.sign(&message).to_bytes()),
        })
    }
    fn verify(&self, manifest: &Manifest) -> Result<()> {
        self.book.validate(manifest)?;
        if self.operator >= 3 || self.signature.len() != 128 {
            return Err("final-book endorsement shape invalid".into());
        }
        let bytes = hex::decode(&self.signature).map_err(|_| "final-book signature encoding")?;
        let signature = Signature::from_slice(&bytes).map_err(|_| "final-book signature size")?;
        manifest.operators()?.keys[self.operator]
            .verify_strict(&self.book.message(), &signature)
            .map_err(|_| "final-book operator signature invalid".into())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Certificate {
    pub endorsements: [Endorsement; 3],
}
impl Certificate {
    pub fn combine(mut endorsements: [Endorsement; 3], manifest: &Manifest) -> Result<Self> {
        endorsements.sort_by_key(|r| r.operator);
        let certificate = Self { endorsements };
        certificate.verify(manifest)?;
        Ok(certificate)
    }
    pub fn verify(&self, manifest: &Manifest) -> Result<()> {
        for (i, endorsement) in self.endorsements.iter().enumerate() {
            endorsement.verify(manifest)?;
            if endorsement.operator != i || endorsement.book != self.endorsements[0].book {
                return Err("final-book quorum is incomplete or inconsistent".into());
            }
        }
        Ok(())
    }
    pub fn check_inclusion(&self, receipt: &AcceptedReceipt, manifest: &Manifest) -> Result<()> {
        self.verify(manifest)?;
        let payload = receipt.payload();
        if !self.endorsements[0].book.entries.contains(payload) {
            return Err("accepted receipt omitted or changed in certified final book".into());
        }
        Ok(())
    }
    /// Verify authentication and exact public-book/refund linkage. Proof validity
    /// and native settlement/consensus verification remain independent checks.
    pub fn verify_statement(&self, encoded: &[u8], manifest: &Manifest) -> Result<()> {
        self.verify(manifest)?;
        if encoded.len() != 232 * 32 {
            return Err("unexpected certified MPC statement shape".into());
        }
        let fields: Vec<[u8; 32]> = encoded
            .chunks_exact(32)
            .map(|b| b.try_into().unwrap())
            .collect();
        for field in &fields {
            canonical_field(field)?;
        }
        let scalar = |value: u64| {
            let mut b = [0; 32];
            b[24..].copy_from_slice(&value.to_be_bytes());
            b
        };
        let mut expected = Vec::new();
        expected.extend(manifest.context.conditions_hash.map(|b| scalar(b as u64)));
        let book = &self.endorsements[0].book;
        for entry in &book.entries {
            expected.extend(entry.ticket.map(|b| scalar(b as u64)));
        }
        let c = &manifest.context;
        expected.extend([
            scalar(c.units as u64),
            scalar(c.reserve),
            scalar(c.budget),
            scalar(c.processing_fee),
        ]);
        expected.extend(book.entries.each_ref().map(|p| p.bid_commitment));
        expected.push(book.accepted_root);
        expected.extend(
            book.entries
                .each_ref()
                .map(|p| p.refund_receiver_commitment),
        );
        expected.extend([
            manifest.seller_receiver_commitment,
            manifest.operator_receiver_commitment,
        ]);
        if fields[..175] != expected {
            return Err("MPC book/root/recipient differs from certified acceptance".into());
        }
        let mut used = [false; 6];
        for field in &fields[175..181] {
            if field[..31] != [0; 31] || field[31] >= 6 || used[field[31] as usize] {
                return Err("MPC action mapping outside fixed shape or duplicated".into());
            }
            used[field[31] as usize] = true;
        }
        for field in &fields[184..] {
            if field[..28] != [0; 28] {
                return Err("MPC row word exceeds u32".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        claim::{Instructions, synthetic_verified_claim_for},
        receipt::Receipt,
    };
    struct Fixture {
        path: std::path::PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "darkcall-final-book-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            Self { path }
        }
        fn journal(&self, i: usize) -> std::path::PathBuf {
            self.path.join(format!("node-{i}.jsonl"))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.path).unwrap();
        }
    }
    fn manifest(keys: &[SigningKey; 3]) -> Manifest {
        Manifest {
            context: Context {
                format: 1,
                network_genesis: [8; 32],
                conditions_hash: [9; 32],
                hash_rule: HASH_RULE.into(),
                units: 3,
                reserve: 20,
                budget: 100,
                processing_fee: 15000,
            },
            operator_keys: keys.each_ref().map(|k| k.verifying_key().to_bytes()),
            closes_at_utc: 100,
            seller_receiver_commitment: [6; 32],
            operator_receiver_commitment: [7; 32],
        }
    }
    fn accept(
        j: &mut Journal,
        m: &Manifest,
        keys: &[SigningKey; 3],
        operator: usize,
        ticket: u8,
    ) -> Receipt {
        let claim = synthetic_verified_claim_for(
            [ticket; 32],
            Instructions {
                bid_commitment: [ticket; 32],
                pair_commitments: [[ticket + 1; 32]; 3],
                refund_receiver_commitment: [ticket + 2; 32],
            },
        );
        let r = claim.registration();
        j.append(Event::Observe {
            note: r.note,
            value: 15100,
            block: [1; 32],
            height: 10,
        })
        .unwrap();
        j.append(Event::Confirm {
            note: r.note,
            block: [1; 32],
        })
        .unwrap();
        j.append(Event::Accept {
            ticket: r.ticket,
            note: r.note,
            bid_commitment: r.bid_commitment,
            receiver_commitment: r.receiver_commitment,
        })
        .unwrap();
        Receipt::issue(
            j,
            &claim,
            &m.operators().unwrap(),
            operator,
            &keys[operator],
        )
        .unwrap()
    }
    fn setup_from(
        first: u8,
    ) -> (
        Fixture,
        [SigningKey; 3],
        Manifest,
        [Journal; 3],
        [AcceptedReceipt; 4],
    ) {
        let f = Fixture::new();
        let keys = [1u8, 2, 3].map(|n| SigningKey::from_bytes(&[n; 32]));
        let m = manifest(&keys);
        let mut journals =
            std::array::from_fn(|i| Journal::open(&f.journal(i), &m.context).unwrap());
        let receipts = std::array::from_fn(|i| {
            let ticket = first + i as u8;
            let claim = synthetic_verified_claim_for(
                [ticket; 32],
                Instructions {
                    bid_commitment: [ticket; 32],
                    pair_commitments: [[ticket + 1; 32]; 3],
                    refund_receiver_commitment: [ticket + 2; 32],
                },
            );
            let reg = claim.registration();
            let endorsements = std::array::from_fn(|operator| {
                let j = &mut journals[operator];
                j.append(Event::Observe {
                    note: reg.note,
                    value: 15100,
                    block: [1; 32],
                    height: 10,
                })
                .unwrap();
                j.append(Event::Confirm {
                    note: reg.note,
                    block: [1; 32],
                })
                .unwrap();
                j.append(Event::Accept {
                    ticket: reg.ticket,
                    note: reg.note,
                    bid_commitment: reg.bid_commitment,
                    receiver_commitment: reg.receiver_commitment,
                })
                .unwrap();
                Receipt::issue(
                    j,
                    &claim,
                    &m.operators().unwrap(),
                    operator,
                    &keys[operator],
                )
                .unwrap()
            });
            AcceptedReceipt::combine(endorsements, &m.operators().unwrap(), &m.context).unwrap()
        });
        (f, keys, m, journals, receipts)
    }
    fn statement(book: &FinalBook, m: &Manifest) -> Vec<u8> {
        let scalar = |v: u64| {
            let mut bytes = [0; 32];
            bytes[24..].copy_from_slice(&v.to_be_bytes());
            bytes
        };
        let mut fields = Vec::new();
        fields.extend(m.context.conditions_hash.map(|b| scalar(b as u64)));
        for entry in &book.entries {
            fields.extend(entry.ticket.map(|b| scalar(b as u64)));
        }
        fields.extend([scalar(3), scalar(20), scalar(100), scalar(15000)]);
        fields.extend(book.entries.each_ref().map(|e| e.bid_commitment));
        fields.push(book.accepted_root);
        fields.extend(
            book.entries
                .each_ref()
                .map(|e| e.refund_receiver_commitment),
        );
        fields.extend([m.seller_receiver_commitment, m.operator_receiver_commitment]);
        fields.extend([5, 2, 0, 4, 1, 3].map(scalar));
        fields.extend([scalar(20), scalar(3), scalar(1)]);
        fields.extend([scalar(0); 48]);
        assert_eq!(fields.len(), 232);
        fields.into_iter().flatten().collect()
    }
    #[test]
    fn three_node_certificate_matches_exact_mpc_and_survives_restart() {
        let (f, keys, m, mut journals, receipts) = setup_from(1);
        let mut reverse = receipts.clone();
        reverse.reverse();
        let book = FinalBook::from_receipts(&m, &reverse).unwrap();
        assert_eq!(book.entries[0].ticket, [1; 32]);
        let endorsements = std::array::from_fn(|i| {
            assert!(Endorsement::issue(&mut journals[i], &book, &m, i, &keys[i], 100).is_err());
            journals[i].append(Event::Freeze).unwrap();
            assert!(Endorsement::issue(&mut journals[i], &book, &m, i, &keys[i], 99).is_err());
            Endorsement::issue(&mut journals[i], &book, &m, i, &keys[i], 100).unwrap()
        });
        let certificate = Certificate::combine(endorsements.clone(), &m).unwrap();
        let encoded = statement(&book, &m);
        certificate.verify_statement(&encoded, &m).unwrap();
        for receipt in &receipts {
            certificate.check_inclusion(receipt, &m).unwrap();
        }
        let (_extra_fixture, _keys, _m, _journals, omitted) = setup_from(5);
        assert!(certificate.check_inclusion(&omitted[0], &m).is_err());
        let wire = serde_json::to_vec(&certificate).unwrap();
        let restored: Certificate = serde_json::from_slice(&wire).unwrap();
        restored.verify_statement(&encoded, &m).unwrap();
        drop(journals);
        let mut j = Journal::open(&f.journal(0), &m.context).unwrap();
        assert_eq!(
            Endorsement::issue(&mut j, &book, &m, 0, &keys[0], 101)
                .unwrap()
                .signature,
            endorsements[0].signature
        );
        let mut changed_manifest = m.clone();
        changed_manifest.closes_at_utc += 1;
        let changed_book = FinalBook::from_receipts(&changed_manifest, &receipts).unwrap();
        assert!(
            Endorsement::issue(&mut j, &changed_book, &changed_manifest, 0, &keys[0], 102).is_err()
        );
        for index in [0, 32, 160, 164, 168, 169, 173, 174, 175, 184] {
            let mut changed = encoded.clone();
            changed[index * 32] ^= 1;
            assert!(
                certificate.verify_statement(&changed, &m).is_err(),
                "field {index}"
            );
        }
        let mut changed = encoded.clone();
        changed[176 * 32..177 * 32].copy_from_slice(&encoded[175 * 32..176 * 32]);
        assert!(certificate.verify_statement(&changed, &m).is_err());
        let mut changed = certificate.clone();
        changed.endorsements[0].signature = "00".repeat(64);
        assert!(changed.verify(&m).is_err());
        assert!(
            Certificate::combine(
                [
                    endorsements[0].clone(),
                    endorsements[0].clone(),
                    endorsements[2].clone()
                ],
                &m
            )
            .is_err()
        );
        let mut changed = m.clone();
        changed.operator_keys.swap(0, 1);
        assert!(certificate.verify(&changed).is_err());
    }
    #[test]
    fn missing_funding_receipt_instructions_and_reorg_cannot_be_certified() {
        let (_f, keys, m, mut journals, receipts) = setup_from(1);
        let book = FinalBook::from_receipts(&m, &receipts).unwrap();
        let mut changed = book.clone();
        changed.entries[0].pair_commitments[0][0] ^= 1;
        journals[0].append(Event::Freeze).unwrap();
        assert!(Endorsement::issue(&mut journals[0], &changed, &m, 0, &keys[0], 100).is_err());
        journals[0]
            .append(Event::Invalidate {
                notes: std::collections::BTreeSet::new(),
            })
            .unwrap();
        assert!(Endorsement::issue(&mut journals[0], &book, &m, 0, &keys[0], 100).is_err());
        accept(&mut journals[1], &m, &keys, 1, 5);
        journals[1].append(Event::Freeze).unwrap();
        assert!(Endorsement::issue(&mut journals[1], &book, &m, 1, &keys[1], 100).is_err());
        let mut alias = book;
        alias.entries[0].bid_commitment = [255; 32];
        assert!(alias.validate(&m).is_err());
        let mut invalid_terms = m.clone();
        invalid_terms.context.processing_fee = 7499;
        assert!(invalid_terms.operators().is_err());
        invalid_terms.context.processing_fee = 15000;
        invalid_terms.context.budget = darkcall_auction_core::MAX_MONEY / 3;
        assert!(invalid_terms.operators().is_err());
    }
}
