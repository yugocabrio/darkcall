//! Actual funded NU7 authorization requires the unchanged application proofs,
//! certified book and exact native effects. No RPC or broadcast capability.
use crate::{Result, auction, funded_prepare, interop_export::write, native, regtest_transfer};
use orchard::{
    circuit::{OrchardCircuitVersion, ProvingKey},
    keys::Scope,
    note::NoteVersion,
};
use pczt::{
    Pczt,
    roles::{
        low_level_signer::Signer,
        prover::Prover,
        tx_extractor::TransactionExtractor,
        verifier::{OrchardError, Verifier},
    },
};
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path, time::Instant};
use zcash_protocol::consensus::BranchId;

#[derive(Deserialize)]
struct OutputMapping {
    outputs: Vec<OutputIndex>,
}
#[derive(Deserialize)]
struct OutputIndex {
    action_index: u32,
}

fn actual<A: zcash_primitives::transaction::Authorization>(
    tx: &zcash_primitives::transaction::TransactionData<A>,
) -> Result<auction::Identity> {
    let mut identity = auction::identity(tx)?;
    identity.scope = funded_prepare::SCOPE.into();
    Ok(identity)
}

pub fn finalize(path: &Path, authorize: bool) -> Result<serde_json::Value> {
    let started = Instant::now();
    let directory = auction::directory(path)?;
    let encoded = auction::read(&directory.join("nu7-prepared.pczt"), 1_000_000)?;
    let pczt = Pczt::parse(&encoded).map_err(|_| "funded NU7 PCZT parse")?;
    if pczt
        .clone()
        .serialize()
        .map_err(|_| "funded PCZT serialization")?
        != encoded
    {
        return Err("funded NU7 PCZT noncanonical or trailing data".into());
    }
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|_| "funded NU7 effects")?;
    let identity = actual(&effects)?;
    let saved: auction::Identity = serde_json::from_slice(&auction::read(
        &directory.join("nu7-native-identity.json"),
        16384,
    )?)
    .map_err(|_| "funded saved identity schema")?;
    if saved != identity {
        return Err("funded prepared native effects changed".into());
    }
    let mut checked = funded_prepare::load(&directory, Some(identity.effects_digest))?;
    let memos = auction::checked_receipt_memos(&directory, authorize)?;
    let bundle = effects.ironwood_bundle().ok_or("funded native bundle")?;
    if bundle.actions().len() != 6
        || i64::from(bundle.value_balance()) != 30000
        || bundle.anchor() != &checked.anchor
    {
        return Err("funded actual native shape/fee/anchor differs".into());
    }
    let real: BTreeSet<_> = bundle
        .actions()
        .iter()
        .filter_map(|a| {
            let n = a.nullifier().to_bytes();
            checked.nullifiers.contains(&n).then_some(n)
        })
        .collect();
    if real != checked.nullifiers
        || bundle
            .actions()
            .iter()
            .filter(|a| checked.nullifiers.contains(&a.nullifier().to_bytes()))
            .count()
            != 4
    {
        return Err("funded actual nullifiers differ from accepted notes".into());
    }
    let mapping: OutputMapping = serde_json::from_slice(&auction::read(
        &directory.join("private-nu7-auction-outputs.json"),
        32768,
    )?)
    .map_err(|_| "funded output mapping schema")?;
    if mapping.outputs.len() != 6 {
        return Err("funded output mapping shape".into());
    }
    let mut indices = BTreeSet::new();
    let ovk = checked.fvk.to_ovk(Scope::External);
    for (i, item) in mapping.outputs.iter().enumerate() {
        let index = item.action_index as usize;
        if index >= 6 || !indices.insert(index) {
            return Err("funded recipient action collision".into());
        }
        let (note, address, memo) = bundle
            .recover_output_with_ovk(index, &ovk)
            .ok_or("funded prepared outgoing note recovery")?;
        if note.value().inner() != checked.request.amounts[i]
            || address != checked.addresses[i]
            || memo != *memos[i].as_array()
            || note.version() != NoteVersion::V3
        {
            return Err("funded prepared recipient/amount/memo substitution".into());
        }
    }
    // Independently verified native actions/effects and all fixed proofs BEFORE
    // reservation or reading the local DKG signing packages. The authority is
    // the configured admission manifest, never one discovered in a bundle.
    eprintln!("Funded NU7 fixed native/MPC/certified-book replay before DKG started");
    let app = if authorize {
        Some(auction::application_for(
            &directory,
            &identity,
            Some(&checked.request.admission.join("manifest.json")),
        )?)
    } else {
        None
    };
    let mut before_fvks = Vec::new();
    let pczt = Verifier::new(pczt)
        .with_ironwood::<String, _>(|b| -> std::result::Result<(), OrchardError<String>> {
            b.verify_cross_address_restriction()?;
            before_fvks = b
                .actions()
                .iter()
                .map(|a| a.spend().fvk().as_ref().map(|k| k.to_bytes()))
                .collect();
            for a in b.actions() {
                let real = checked
                    .nullifiers
                    .contains(&a.spend().nullifier().to_bytes());
                if a.spend()
                    .value()
                    .as_ref()
                    .is_none_or(|v| v.inner() != if real { 15100 } else { 0 })
                    || (real == a.spend().spend_auth_sig().is_some())
                {
                    return Err(OrchardError::Custom(
                        "funded real/dummy input value or authorization mismatch".into(),
                    ));
                }
                a.verify_cv_net()?;
                a.spend().verify_nullifier(Some(&checked.fvk))?;
                a.spend().verify_rk(Some(&checked.fvk))?;
                a.output().verify_note_commitment(a.spend())?;
            }
            Ok(())
        })
        .map_err(|_| "funded NU7 native private-field rejection")?
        .finish();
    for j in &checked.journals {
        j.reservation_needed(identity.effects_digest, false)?;
    }
    if !authorize {
        return Ok(
            serde_json::json!({"scope":"actual funded NU7 prepared PCZT byte/claim/memo verification only",
            "txid":identity.txid,"four_actual_inputs_match_frozen_or_exact_prepared_book":true,
            "six_recipients_amounts_memos_match":true,"pczt_canonical":true,
            "proving_keys_or_real_signatures_created":false,"funding_reservations_changed":false,"broadcast":false}),
        );
    }
    let mut reservation_writes = 0;
    for j in &mut checked.journals {
        reservation_writes += usize::from(j.reserve_transaction(identity.effects_digest, false)?);
    }
    let (keys, dkg) = regtest_transfer::signing_keys(&checked.request.escrow, &checked.fvk)?;
    if !dkg {
        return Err("funded auction requires existing DKG, never dealer fallback".into());
    }
    let mut rng = rand::rng();
    let mut old_rng = rand_old::rngs::OsRng;
    let proof_start = Instant::now();
    let pk = ProvingKey::build(OrchardCircuitVersion::PostNu6_3);
    let pczt = Prover::new(pczt)
        .create_ironwood_proof(&mut rng, &pk)
        .map_err(|_| "funded NU7 native proof")?
        .finish();
    let proof_seconds = proof_start.elapsed().as_secs_f64();
    if actual(
        &pczt
            .clone()
            .into_effects()
            .map_err(|_| "funded pre-sign effects")?,
    )? != identity
    {
        return Err("funded effects changed before DKG signing".into());
    }
    let mut spends = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<native::SignError, _>(|_, b, _| {
            for a in b.actions_mut() {
                let is_real = checked
                    .nullifiers
                    .contains(&a.spend().nullifier().to_bytes());
                if is_real && a.spend().spend_auth_sig().is_some() {
                    return Err(native::SignError::Custom(
                        "funded real spend was pre-authorized".into(),
                    ));
                }
                if a.spend().spend_auth_sig().is_none() {
                    if !is_real {
                        return Err(native::SignError::Custom(
                            "funded unexpected unsigned input".into(),
                        ));
                    }
                    let alpha = a
                        .spend()
                        .alpha()
                        .as_ref()
                        .ok_or_else(|| native::SignError::Custom("funded alpha".into()))?;
                    let sig = native::threshold_signature(
                        &keys,
                        alpha,
                        &identity.effects_digest,
                        &mut old_rng,
                    )
                    .map_err(native::SignError::Custom)?;
                    let mut changed = identity.effects_digest;
                    changed[0] ^= 1;
                    if a.spend().rk().verify(&changed, &sig).is_ok() {
                        return Err(native::SignError::Custom(
                            "funded changed digest accepted".into(),
                        ));
                    }
                    a.apply_signature(identity.effects_digest, sig)
                        .map_err(native::SignError::Apply)?;
                    spends += 1;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .finish();
    if spends != 4 {
        return Err("funded four threshold spends required".into());
    }
    let pczt = Verifier::new(pczt)
        .with_ironwood::<String, _>(|b| -> std::result::Result<(), OrchardError<String>> {
            let after: Vec<_> = b
                .actions()
                .iter()
                .map(|a| a.spend().fvk().as_ref().map(|k| k.to_bytes()))
                .collect();
            if after != before_fvks {
                return Err(OrchardError::Custom(
                    "funded signer changed FVK bytes".into(),
                ));
            }
            Ok(())
        })
        .map_err(|_| "funded signer FVK preservation")?
        .finish();
    let tx = TransactionExtractor::new(pczt)
        .extract(&mut rng)
        .map_err(|_| "funded NU7 extraction")?;
    if actual(&tx)? != identity {
        return Err("funded extracted effects changed".into());
    }
    for (i, item) in mapping.outputs.iter().enumerate() {
        let (note, address, memo) = tx
            .ironwood_bundle()
            .ok_or("funded final bundle")?
            .recover_output_with_ovk(item.action_index as usize, &ovk)
            .ok_or("funded final output recovery")?;
        if note.value().inner() != checked.request.amounts[i]
            || address != checked.addresses[i]
            || memo != *memos[i].as_array()
        {
            return Err("funded final output substitution".into());
        }
    }
    let mut bytes = Vec::new();
    tx.write(&mut bytes)
        .map_err(|_| "funded NU7 transaction serialization")?;
    native::verify_bytes(&bytes, BranchId::Nu7, &mut rng)?;
    write(&directory.join("funded-nu7-transaction.bin"), &bytes)?;
    let result = serde_json::json!({"scope":"actual-note certified NU7 auction construction; not broadcast",
        "txid":tx.txid().to_string(),"transaction_bytes":bytes.len(),"fee":30000,"threshold_spends":spends,
        "dkg_source":true,"master_spending_key_read":false,"full_ask_reconstructed":false,
        "setup":"existing local 2-of-3 DKG; all shares on one host","application":app,
        "native_proof_and_signatures_verified":true,"four_receipt_memos_in_same_six_output_transaction":true,
        "six_outgoing_notes_recovered":true,"recipient_keys_read":false,"pczt_fvks_preserved":true,
        "one_share_rejected":true,"changed_digest_rejected":true,"funding_reservation_writes":reservation_writes,
        "proof_seconds":proof_seconds,"seconds":started.elapsed().as_secs_f64(),"broadcast":false});
    write(
        &directory.join("nu7-funded-construction-summary.json"),
        &serde_json::to_vec(&result).unwrap(),
    )?;
    Ok(result)
}

pub fn verify(path: &Path, authority: &Path) -> Result<serde_json::Value> {
    let started = Instant::now();
    let directory = auction::directory(path)?;
    let bytes = auction::read(&directory.join("funded-nu7-transaction.bin"), 100000)?;
    let tx = native::verify_bytes(&bytes, BranchId::Nu7, &mut rand::rng())?;
    let identity = actual(&tx)?;
    let saved: auction::Identity = serde_json::from_slice(&auction::read(
        &directory.join("nu7-native-identity.json"),
        16384,
    )?)
    .map_err(|_| "funded public identity schema")?;
    if saved != identity {
        return Err("funded public native effects identity mismatch".into());
    }
    let app = auction::application_for(&directory, &identity, Some(authority))?;
    Ok(
        serde_json::json!({"scope":"public-only certified NU7 native/MPC/row replay; chain inclusion checked separately",
        "txid":tx.txid().to_string(),"native_proof_and_signatures_verified":true,"application":app,
        "private_openings_keys_pczt_read":false,"actual_transaction_outputs_match":true,
        "transaction_bytes":bytes.len(),"seconds":started.elapsed().as_secs_f64(),
        "chain_funding_or_inclusion_verified":false,"broadcast":false}),
    )
}

/// Lightweight accepted-book linkage for receipt clients. This does not claim
/// that the caller has verified the native row proofs or transaction signatures.
pub fn certificate_link(path: &Path, authority: &Path) -> Result<serde_json::Value> {
    use darkcall_wallet_protocol::final_book::{Certificate, Manifest};
    let directory = auction::directory(path)?;
    let certificate: Certificate = serde_json::from_slice(&auction::read(
        &directory.join("final-book-certificate.json"),
        131072,
    )?)
    .map_err(|_| "funded certificate schema")?;
    let manifest: Manifest = serde_json::from_slice(&auction::read(authority, 16384)?)
        .map_err(|_| "strict caller-trusted funded authority")?;
    let result: serde_json::Value =
        serde_json::from_slice(&auction::read(&directory.join("mpc-result.json"), 4096)?)
            .map_err(|_| "funded MPC metadata schema")?;
    let mpc = auction::directory(Path::new(result["run"].as_str().ok_or("funded MPC path")?))?;
    certificate.verify_statement(
        &auction::read(&mpc.join("public-input0"), 232 * 32)?,
        &manifest,
    )?;
    Ok(
        serde_json::json!({"accepted_book_and_recipients_authenticated":true,
        "authority_source":"explicit caller configuration","native_row_or_auction_proof_verified_here":false}),
    )
}
