//! Four real encrypted deposit outputs from an actual regtest source note.
//! Client keys are local demo actors; no maximum bids or oracle fills are read.
use crate::{
    Result, auction,
    claims::verify_output,
    interop_export::write as write_private,
    native,
    regtest_transfer::{Regtest, load_source, signing_keys},
    regtest_wallet,
};
use group::GroupEncoding;
use orchard::circuit::{OrchardCircuitVersion, ProvingKey};
use orchard::keys::Scope;
use pczt::roles::{
    creator::Creator, io_finalizer::IoFinalizer, low_level_signer::Signer, prover::Prover,
    tx_extractor::TransactionExtractor,
};
use rand_old::{RngCore, rngs::OsRng};
use std::{convert::Infallible, time::Instant};
use zcash_primitives::transaction::{
    TxVersion,
    builder::{BuildConfig, Builder, BundlePadding},
    fees::zip317::FeeRule,
    sighash::SignableInput,
    sighash_v6::v6_signature_hash,
    txid::TxIdDigester,
};
use zcash_protocol::{consensus::BranchId, memo::MemoBytes, value::Zatoshis};
fn read_private(path: &Path, limit: u64) -> Result<Vec<u8>> {
    auction::directory(path.parent().ok_or("NU7 deposit input parent")?)?;
    auction::read(path, limit)
}
fn error(context: &str, error: impl std::fmt::Debug) -> String {
    format!("{context}: {error:?}")
}

use darkcall_input_tool::{context, decimal_field_bytes, receiver_commitment};
use darkcall_wallet_protocol::claim::{Claim, DepositMemo, ExpectedFunding, Instructions};
use ed25519_dalek::SigningKey;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    pub condition_digest: [u8; 32],
    pub ids: [[u8; 32]; 4],
    pub active: [bool; 4],
    pub units: u32,
    pub reserve: u64,
    pub budget: u64,
    pub processing_fee: u64,
    pub commitments: [String; 4],
    pub pair_commitments: [[String; 3]; 4],
    pub root: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receiver {
    receiver: Vec<u8>,
    nonce: [u8; 32],
}
fn fresh() -> [u8; 32] {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}
pub fn build(
    wallet: &Path,
    openings: &Path,
    escrow_wallet: &Path,
    book: &Path,
    receivers: &Path,
    name: &str,
) -> Result<serde_json::Value> {
    let start = Instant::now();
    let mut rng = rand::rng();
    if name.is_empty()
        || name.len() > 80
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("expected a fresh simple local name".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let out = root.join(".local").join(name);
    if out.try_exists().map_err(|e| error("output path", e))? {
        return Err("deposit output already exists".into());
    }
    let source = load_source(wallet, openings)?;
    let escrow = regtest_wallet::viewing_key(escrow_wallet)?;
    let escrow_signing_key_read = escrow_wallet
        .join("private-spending-key.bin")
        .try_exists()
        .map_err(|e| error("escrow viewing setup", e))?;
    let book: Book =
        serde_json::from_slice(&read_private(book, 16384)?).map_err(|e| error("book", e))?;
    let receivers: [Receiver; 6] = serde_json::from_slice(&read_private(receivers, 16384)?)
        .map_err(|e| error("receivers", e))?;
    let mut sorted = book.ids;
    sorted.sort();
    if book.active != [true; 4]
        || book.ids != sorted
        || sorted.windows(2).any(|ids| ids[0] == ids[1])
    {
        return Err("expected four canonical distinct active tickets".into());
    }
    let bid_commitments = book.commitments.each_ref().map(|s| decimal_field_bytes(s));
    let bid_commitments: [[u8; 32]; 4] = bid_commitments
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?
        .try_into()
        .unwrap();
    if darkcall_input_tool::four_active_root(
        &book.condition_digest,
        book.units,
        book.reserve,
        book.budget,
        book.processing_fee,
        bid_commitments,
    )? != decimal_field_bytes(&book.root)?
    {
        return Err("public book root invalid".into());
    }
    let ctx = context(
        &book.condition_digest,
        book.units,
        book.reserve,
        book.budget,
        book.processing_fee,
    );
    let mut receiver_commitments = Vec::new();
    for (i, receiver) in receivers.iter().enumerate() {
        let encoded: [u8; 43] = receiver
            .receiver
            .as_slice()
            .try_into()
            .map_err(|_| "receiver length")?;
        let address = orchard::Address::from_raw_address_bytes(&encoded)
            .into_option()
            .ok_or("invalid receiver")?;
        let ticket = if i < 4 { book.ids[i] } else { [0; 32] };
        let role = if i < 4 {
            0
        } else if i == 4 {
            1
        } else {
            2
        };
        receiver_commitments.push(receiver_commitment(
            ctx,
            &ticket,
            role,
            &address.g_d().to_bytes(),
            &address.pk_d().inner().to_bytes(),
            &receiver.nonce,
        )?);
    }
    let genesis: [u8; 32] =
        hex::decode("029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327")
            .unwrap()
            .try_into()
            .unwrap();
    let clients: [SigningKey; 4] = std::array::from_fn(|_| SigningKey::from_bytes(&fresh()));
    let memos: [DepositMemo; 4] = std::array::from_fn(|i| DepositMemo {
        network_genesis: genesis,
        conditions: book.condition_digest,
        ticket: book.ids[i],
        client_key: clients[i].verifying_key().to_bytes(),
        nonce: fresh(),
    });
    let required = book
        .budget
        .checked_add(book.processing_fee)
        .ok_or("deposit overflow")?;
    let fee = 25_000u64;
    let change = source
        .note
        .value()
        .inner()
        .checked_sub(
            required
                .checked_mul(4)
                .and_then(|n| n.checked_add(fee))
                .ok_or("deposit total overflow")?,
        )
        .ok_or("insufficient actual funds")?;
    let anchor = source.path.root(source.note.commitment().into());
    let mut builder = Builder::new(
        Regtest,
        zcash_protocol::consensus::BlockHeight::from_u32(
            source.tip_height.checked_add(1).ok_or("height overflow")?,
        ),
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
            ironwood_anchor: Some(anchor),
            orchard_padding: BundlePadding::DEFAULT,
            ironwood_padding: BundlePadding::DEFAULT,
        },
    );
    builder
        .add_ironwood_spend::<Infallible>(source.fvk.clone(), source.note, source.path)
        .map_err(|e| error("deposit source", e))?;
    for memo in &memos {
        builder
            .add_ironwood_output::<Infallible>(
                None,
                escrow.address_at(0u32, Scope::External),
                Zatoshis::from_u64(required).map_err(|e| error("deposit amount", e))?,
                MemoBytes::from_bytes(&memo.encode()?).map_err(|e| error("deposit memo", e))?,
            )
            .map_err(|e| error("deposit output", e))?;
    }
    builder
        .add_ironwood_output::<Infallible>(
            None,
            source.fvk.address_at(1u32, Scope::Internal),
            Zatoshis::from_u64(change).map_err(|e| error("change", e))?,
            MemoBytes::empty(),
        )
        .map_err(|e| error("change output", e))?;
    let parts = builder
        .build_for_pczt(&mut rng, &FeeRule::standard())
        .map_err(|e| error("deposit PCZT", e))?;
    let indices: [usize; 4] =
        std::array::from_fn(|i| parts.ironwood_meta.output_action_index(i).unwrap());
    let pczt =
        IoFinalizer::new(Creator::build_from_parts(parts.pczt_parts).ok_or("deposit creator")?)
            .finalize_io(&mut rng)
            .map_err(|e| error("deposit IO", e))?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|e| error("deposit effects", e))?;
    if effects.version() != TxVersion::V6
        || effects.consensus_branch_id() != BranchId::Nu7
        || effects.transparent_bundle().is_some()
        || effects.sapling_bundle().is_some()
        || effects.orchard_bundle().is_some()
        || effects
            .ironwood_bundle()
            .ok_or("deposit bundle")?
            .actions()
            .len()
            != 5
    {
        return Err("deposit shape differs".into());
    }
    let mut claims = Vec::new();
    for i in 0..4 {
        let pairs = book.pair_commitments[i]
            .each_ref()
            .map(|s| decimal_field_bytes(s))
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let claim = Claim::sign(
            &memos[i],
            effects.ironwood_bundle().unwrap().actions()[indices[i]]
                .cmx()
                .to_bytes(),
            Instructions {
                bid_commitment: bid_commitments[i],
                pair_commitments: pairs.try_into().unwrap(),
                refund_receiver_commitment: receiver_commitments[i],
            },
            &clients[i],
        )?;
        verify_output(
            &effects,
            indices[i],
            &escrow,
            &ExpectedFunding {
                network_genesis: genesis,
                conditions: book.condition_digest,
                ticket: book.ids[i],
                required_value: required,
            },
            &claim,
        )?;
        claims.push(claim.to_wire());
    }
    let message: [u8; 32] = v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
    .try_into()
    .unwrap();
    let (keys, source_is_dkg) = signing_keys(wallet, &source.fvk)?;
    let proof_start = Instant::now();
    let pczt = Prover::new(pczt)
        .create_ironwood_proof(
            &mut rng,
            &ProvingKey::build(OrchardCircuitVersion::PostNu6_3),
        )
        .map_err(|e| error("deposit proof", e))?
        .finish();
    let proof_seconds = proof_start.elapsed().as_secs_f64();
    let mut signed = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<native::SignError, _>(|_, bundle, _| {
            for action in bundle.actions_mut() {
                if action.spend().spend_auth_sig().is_none() {
                    let alpha =
                        action.spend().alpha().as_ref().ok_or_else(|| {
                            native::SignError::Custom("missing deposit alpha".into())
                        })?;
                    let signature = native::threshold_signature(&keys, alpha, &message, &mut OsRng)
                        .map_err(native::SignError::Custom)?;
                    action
                        .apply_signature(message, signature)
                        .map_err(native::SignError::Apply)?;
                    signed += 1;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .finish();
    if signed != 1 {
        return Err("deposit source signing count".into());
    }
    let tx = TransactionExtractor::new(pczt)
        .extract(&mut rng)
        .map_err(|e| error("deposit extraction", e))?;
    if tx.txid().as_ref() != &message {
        return Err("deposit identity changed".into());
    }
    let mut bytes = Vec::new();
    tx.write(&mut bytes)
        .map_err(|e| error("deposit bytes", e))?;
    native::verify_bytes(&bytes, BranchId::Nu7, &mut rng)?;
    fs::create_dir(&out).map_err(|e| error("deposit directory", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&out, fs::Permissions::from_mode(0o700))
            .map_err(|e| error("deposit permissions", e))?;
    }
    write_private(&out.join("nu7-regtest-transaction.bin"), &bytes)?;
    write_private(
        &out.join("private-claims.json"),
        &serde_json::to_vec(&claims).map_err(|e| error("claim encode", e))?,
    )?;
    write_private(&out.join("private-deposit-map.json"),&serde_json::to_vec(&serde_json::json!({"indices":indices,"receiver_commitments":receiver_commitments,"source_index":source.index})).map_err(|e|error("map encode",e))?)?;
    fs::File::open(&out)
        .and_then(|f| f.sync_all())
        .map_err(|e| error("deposit directory sync", e))?;
    let summary = serde_json::json!({"scope":"four actual NU7-regtest encrypted claim deposits; fixture tickets","branch":"77190ad9","source_is_dkg":source_is_dkg,"txid":tx.txid().to_string(),"deposit_outputs":4,"native_actions":5,"transaction_bytes":bytes.len(),"deposit_value":required,"fee":fee,"maximum_bids_read":false,"client_signatures_generated":4,"keys_or_memos_logged":false,"native_proof_and_signatures_verified":true,"proof_seconds":proof_seconds,"seconds":start.elapsed().as_secs_f64(),"broadcast":false,"custody_setup":if escrow_signing_key_read {"owned bank wallet signs deposits; historical escrow viewing key derived from retained spending key"} else {"owned bank wallet signs deposits; DKG/view-only escrow destination full viewing key"},"escrow_signing_key_read":escrow_signing_key_read});
    write_private(
        &out.join("summary.json"),
        &serde_json::to_vec_pretty(&summary).map_err(|e| error("deposit summary", e))?,
    )?;
    Ok(summary)
}
