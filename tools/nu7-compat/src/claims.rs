//! Existing client-intent authentication after actual NU7 ciphertext decryption.
//! No acceptance of exported plaintext as funding evidence, and no chain proof.
use crate::Result;
use darkcall_wallet_protocol::claim::{
    Claim, ExpectedFunding, VerifiedClaim, verify_decrypted_note,
};
use orchard::keys::{FullViewingKey, Scope};
use zcash_primitives::transaction::{Authorization, TransactionData, TxVersion};
use zcash_protocol::consensus::BranchId;

/// Read-only claim replay for actual encrypted deposit transactions. It does
/// not create receipts, journal entries, keys, proofs or funding observations.
pub fn check_deposit(
    escrow: &std::path::Path,
    deposit: &std::path::Path,
) -> Result<serde_json::Value> {
    use darkcall_input_tool::decimal_field_bytes;
    use darkcall_wallet_protocol::claim::WireClaim;
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Mapping {
        indices: [usize; 4],
        receiver_commitments: [[u8; 32]; 6],
        source_index: usize,
    }
    let directory = crate::auction::directory(deposit)?;
    let book: crate::regtest_deposits::Book = serde_json::from_slice(&crate::auction::read(
        &directory.join("private-book.json"),
        16384,
    )?)
    .map_err(|_| "NU7 replay book schema")?;
    let mapping: Mapping = serde_json::from_slice(&crate::auction::read(
        &directory.join("private-deposit-map.json"),
        16384,
    )?)
    .map_err(|_| "NU7 replay map schema")?;
    let _source_index = mapping.source_index;
    let claims: [WireClaim; 4] = serde_json::from_slice(&crate::auction::read(
        &directory.join("private-claims.json"),
        65536,
    )?)
    .map_err(|_| "NU7 replay claims schema")?;
    let bytes = crate::auction::read(&directory.join("nu7-regtest-transaction.bin"), 1000000)?;
    let mut cursor = std::io::Cursor::new(&bytes);
    let tx = zcash_primitives::transaction::Transaction::read(&mut cursor, BranchId::Nu7)
        .map_err(|_| "NU7 replay native transaction parse")?;
    if cursor.position() != bytes.len() as u64 {
        return Err("NU7 replay native transaction trailing bytes".into());
    }
    let key = crate::regtest_wallet::viewing_key(escrow)?;
    let expected_value = book
        .budget
        .checked_add(book.processing_fee)
        .ok_or("NU7 replay amount overflow")?;
    let genesis = hex::decode("029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327")
        .unwrap()
        .try_into()
        .unwrap();
    for (i, wire) in claims.iter().enumerate() {
        let verified = verify_output(
            &tx,
            mapping.indices[i],
            &key,
            &ExpectedFunding {
                network_genesis: genesis,
                conditions: book.condition_digest,
                ticket: book.ids[i],
                required_value: expected_value,
            },
            &Claim::from_wire(wire.clone())?,
        )?;
        let expected_pairs = book.pair_commitments[i]
            .each_ref()
            .map(|s| decimal_field_bytes(s))
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if verified.registration().bid_commitment != decimal_field_bytes(&book.commitments[i])?
            || verified.registration().receiver_commitment != mapping.receiver_commitments[i]
            || verified.pair_commitments().as_slice() != expected_pairs.as_slice()
        {
            return Err("NU7 replay signed instructions differ from declared book/receiver".into());
        }
    }
    Ok(
        serde_json::json!({"scope":"read-only actual NU7 deposit ciphertext/client-intent check",
        "native_outputs_decrypted":4,"client_claim_signatures_verified":4,
        "receipts_or_journals_created":false,"chain_inclusion_or_unspent_verified":false,
        "client_zk_or_delivered_shares_verified":false,"maximum_bids_read":false}),
    )
}

pub fn verify_output<A: Authorization>(
    tx: &TransactionData<A>,
    action_index: usize,
    escrow: &FullViewingKey,
    expected: &ExpectedFunding,
    claim: &Claim,
) -> Result<VerifiedClaim> {
    if tx.version() != TxVersion::V6 || tx.consensus_branch_id() != BranchId::Nu7 {
        return Err("claim adapter requires actual NU7 v6 effects".into());
    }
    let bundle = tx.ironwood_bundle().ok_or("NU7 funding bundle missing")?;
    let action = bundle
        .actions()
        .get(action_index)
        .ok_or("NU7 funding action index out of range")?;
    for scope in [Scope::External, Scope::Internal] {
        if let Some((note, _, memo)) =
            bundle.decrypt_output_with_key(action_index, &escrow.to_ivk(scope))
        {
            if note.version() != orchard::note::NoteVersion::V3 {
                return Err("NU7 funding note must be V3".into());
            }
            let address = orchard_old::Address::from_raw_address_bytes(
                &note.recipient().to_raw_address_bytes(),
            )
            .into_option()
            .ok_or("NU7 claim recipient byte compatibility")?;
            let rho = orchard_old::note::Rho::from_bytes(&note.rho().to_bytes())
                .into_option()
                .ok_or("NU7 claim rho byte compatibility")?;
            let seed = orchard_old::note::RandomSeed::from_bytes(*note.rseed().as_bytes(), &rho)
                .into_option()
                .ok_or("NU7 claim seed byte compatibility")?;
            let old = orchard_old::Note::from_parts(
                address,
                orchard_old::value::NoteValue::from_raw(note.value().inner()),
                rho,
                seed,
                orchard_old::note::NoteVersion::V3,
            )
            .into_option()
            .ok_or("NU7 claim note byte compatibility")?;
            if orchard_old::note::ExtractedNoteCommitment::from(old.commitment()).to_bytes()
                != action.cmx().to_bytes()
            {
                return Err(
                    "NU7 claim reconstructed commitment differs from actual ciphertext output"
                        .into(),
                );
            }
            return verify_decrypted_note(&old, &memo, expected, claim);
        }
    }
    Err("NU7 funding ciphertext cannot be decrypted by this escrow".into())
}
