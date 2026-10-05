//! Native NU6.3 ciphertext adapter around the shared client-intent validation.
#[path = "claim_intent.rs"]
mod intent;
pub use intent::*;
use orchard::keys::{FullViewingKey, Scope};
use zcash_primitives::transaction::{Authorization, TransactionData, TxVersion};
use zcash_protocol::consensus::BranchId;
type Result<T> = std::result::Result<T, String>;
/// The caller separately verifies inclusion, unspent status and consensus for
/// this exact transaction/action against the compact scanner. This function
/// decrypts the actual full ciphertext with the escrow viewing key and validates
/// who may supply bid/refund instructions. An effects-only fixture can test
/// decryption; it is not accepted as on-chain funding merely by this function.
pub fn verify_output<A: Authorization>(
    tx: &TransactionData<A>,
    action_index: usize,
    escrow: &FullViewingKey,
    expected: &ExpectedFunding,
    claim: &Claim,
) -> Result<VerifiedClaim> {
    if tx.version() != TxVersion::V6 || tx.consensus_branch_id() != BranchId::Nu6_3 {
        return Err("claim verifier requires the pinned Ironwood v6 branch".into());
    }
    let bundle = tx
        .ironwood_bundle()
        .ok_or("missing Ironwood funding bundle")?;
    for scope in [Scope::External, Scope::Internal] {
        if let Some((note, _, memo)) =
            bundle.decrypt_output_with_key(action_index, &escrow.to_ivk(scope))
        {
            return verify_decrypted_note(&note, &memo, expected, claim);
        }
    }
    Err("funding output is not decryptable by the escrow viewing key".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use orchard::{
        Note,
        keys::SpendingKey,
        note::{NoteVersion, RandomSeed, Rho},
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
    fn actual_ironwood_ciphertext_authenticates_deposit_claim() {
        use incrementalmerkletree::Hashable;
        use orchard::{
            builder::BundleType,
            tree::{MerkleHashOrchard, MerklePath},
        };
        use pczt::roles::{creator::Creator, io_finalizer::IoFinalizer};
        use zcash_primitives::transaction::{
            builder::{BuildConfig, Builder},
            fees::zip317::FeeRule,
        };
        use zcash_protocol::{
            consensus::{NetworkUpgrade, Parameters, TEST_NETWORK},
            memo::MemoBytes,
            value::Zatoshis,
        };
        let (old_note, memo, key, instructions, expected) = fixture();
        let escrow = FullViewingKey::from(&SpendingKey::from_bytes([1; 32]).unwrap());
        let input = Note::from_parts(
            old_note.recipient(),
            NoteValue::from_raw(30000),
            old_note.rho(),
            *old_note.rseed(),
            NoteVersion::V3,
        )
        .unwrap();
        let path = MerklePath::from_parts(
            0,
            std::array::from_fn(|level| MerkleHashOrchard::empty_root((level as u8).into())),
        );
        let height = TEST_NETWORK
            .activation_height(NetworkUpgrade::Nu6_3)
            .unwrap()
            + 1;
        let mut builder = Builder::new(
            TEST_NETWORK,
            height,
            BuildConfig::Standard {
                sapling_anchor: None,
                orchard_anchor: None,
                ironwood_anchor: Some(path.root(input.commitment().into())),
                orchard_pool_bundle_type: BundleType::DEFAULT,
            },
        );
        builder
            .add_ironwood_spend::<std::convert::Infallible>(escrow.clone(), input, path)
            .unwrap();
        builder
            .add_ironwood_output::<std::convert::Infallible>(
                None,
                escrow.address_at(0u32, Scope::External),
                Zatoshis::from_u64(15100).unwrap(),
                MemoBytes::from_bytes(&memo.encode().unwrap()).unwrap(),
            )
            .unwrap();
        builder
            .add_ironwood_output::<std::convert::Infallible>(
                None,
                escrow.address_at(1u32, Scope::Internal),
                Zatoshis::from_u64(4900).unwrap(),
                MemoBytes::empty(),
            )
            .unwrap();
        let parts = builder.build_for_pczt(OsRng, &FeeRule::standard()).unwrap();
        let index = parts.ironwood_meta.output_action_index(0).unwrap();
        let effects = IoFinalizer::new(Creator::build_from_parts(parts.pczt_parts).unwrap())
            .finalize_io()
            .unwrap()
            .into_effects()
            .unwrap();
        let note_id = effects.ironwood_bundle().unwrap().actions()[index]
            .cmx()
            .to_bytes();
        let claim = Claim::sign(&memo, note_id, instructions, &key).unwrap();
        let verified = verify_output(&effects, index, &escrow, &expected, &claim).unwrap();
        assert_eq!(verified.registration().note, note_id);
        let other = FullViewingKey::from(&SpendingKey::from_bytes([2; 32]).unwrap());
        assert!(verify_output(&effects, index, &other, &expected, &claim).is_err());
        let other_index = 1 - index;
        assert!(verify_output(&effects, other_index, &escrow, &expected, &claim).is_err());
    }
}
