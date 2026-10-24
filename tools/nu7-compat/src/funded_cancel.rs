//! Cancellation of a funded, certified book that cannot settle, for example
//! because one operator stopped and clearing needs all three. Every accepted
//! deposit goes back to its ticket's certified refund receiver, less an equal
//! share of the network fee, signed 2-of-3 by two named operators; the third
//! operator's journal and key share are never opened. There is no auction
//! result, so no auction or output-binding proof is involved: each bidder
//! checks its own refund in its own wallet. Each journal reserves one
//! transaction for good, so once two operators reserve this refund no two
//! operators can sign a settlement of the same deposits, and the reverse.
use crate::{
    Result, auction,
    funded_prepare::{Receiver, json},
    interop_export::{SDK, write},
    native,
    regtest_transfer::{Regtest, Source, load_selected, signing_keys_of},
    regtest_wallet,
};
use darkcall_auction_core::funding::Phase;
use darkcall_input_tool::{context, receiver_commitment};
use darkcall_wallet_protocol::{
    final_book::{Certificate, Manifest},
    journal::Journal,
};
use group::GroupEncoding;
use orchard::{
    circuit::{OrchardCircuitVersion, ProvingKey},
    keys::Scope,
    note::{ExtractedNoteCommitment, NoteVersion},
};
use pczt::{
    Pczt,
    roles::{
        creator::Creator,
        io_finalizer::IoFinalizer,
        low_level_signer::Signer,
        prover::Prover,
        tx_extractor::TransactionExtractor,
        verifier::{OrchardError, Verifier},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    convert::Infallible,
    path::{Path, PathBuf},
    time::Instant,
};
use zcash_primitives::transaction::{
    Authorization, TransactionData, TxVersion,
    builder::{BuildConfig, Builder, BundlePadding},
    fees::zip317::FeeRule,
    sighash::SignableInput,
    sighash_v6::v6_signature_hash,
    txid::{TxIdDigester, to_txid},
};
use zcash_protocol::{
    consensus::{BlockHeight, BranchId},
    memo::MemoBytes,
    value::Zatoshis,
};

pub const SCOPE: &str = "actual NU7 cancellation refund of a funded certified book";
/// ZIP-317: four spends and four outputs are four logical actions at 5,000 each.
const FEE: u64 = 20000;
const MEMO: &[u8] = b"DarkCall: this auction was cancelled before settlement. Your deposit is refunded in full, less a 5,000 zatoshi share of the network fee.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: u32,
    scope: String,
    sdk: String,
    branch: String,
    escrow: PathBuf,
    notes: PathBuf,
    receivers: PathBuf,
    admission: PathBuf,
    signers: [usize; 2],
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Identity {
    format: u32,
    scope: String,
    sdk: String,
    branch: String,
    txid: String,
    effects_digest: [u8; 32],
    native_actions: [[u8; 32]; 4],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    ticket: usize,
    action_index: u32,
    amount: u64,
}
struct Checked {
    request: Request,
    fvk: orchard::keys::FullViewingKey,
    addresses: Vec<orchard::Address>,
    journals: Vec<Journal>,
    inputs: Vec<Source>,
    anchor: orchard::Anchor,
    nullifiers: BTreeSet<[u8; 32]>,
    refund: u64,
}

fn identity<A: Authorization>(tx: &TransactionData<A>) -> Result<Identity> {
    if tx.version() != TxVersion::V6
        || tx.consensus_branch_id() != BranchId::Nu7
        || tx.transparent_bundle().is_some()
        || tx.sapling_bundle().is_some()
        || tx.orchard_bundle().is_some()
    {
        return Err("cancellation transaction pools/version/branch".into());
    }
    let native_actions = tx
        .ironwood_bundle()
        .ok_or("cancellation native bundle")?
        .actions()
        .iter()
        .map(|a| a.cmx().to_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "cancellation expects four actions")?;
    let txid = to_txid(
        tx.version(),
        tx.consensus_branch_id(),
        &tx.digest(TxIdDigester),
    );
    Ok(Identity {
        format: 1,
        scope: SCOPE.into(),
        sdk: SDK.into(),
        branch: "77190ad9".into(),
        txid: txid.to_string(),
        effects_digest: *txid.as_ref(),
        native_actions,
    })
}

fn memo() -> Result<MemoBytes> {
    MemoBytes::from_bytes(MEMO).map_err(|_| "cancellation memo".into())
}

/// Everything an honest signer checks before a refund: the certified book, its
/// four accepted escrow notes, each ticket's certified refund receiver, and that
/// the two named operators' journals hold no other reservation.
fn load(directory: &Path, reserved: Option<[u8; 32]>) -> Result<Checked> {
    let req: Request = json(&directory.join("nu7-cancel-request.json"), 32768)?;
    if req.format != 1
        || req.scope != SCOPE
        || req.sdk != SDK
        || req.branch != "77190ad9"
        || req.signers[0] >= 3
        || req.signers[1] >= 3
        || req.signers[0] == req.signers[1]
    {
        return Err("cancellation request scope/pin/signers mismatch".into());
    }
    let fvk = regtest_wallet::viewing_key(&req.escrow)?;
    if req.escrow.join("private-spending-key.bin").exists() {
        return Err("cancellation requires the DKG view-only escrow".into());
    }
    let manifest: Manifest = json(&req.admission.join("manifest.json"), 16384)?;
    let certificate: Certificate =
        json(&req.admission.join("final-book-certificate.json"), 131072)?;
    certificate.verify(&manifest)?;
    let c = &manifest.context;
    let book = &certificate.endorsements[0].book;
    let chain = crate::network::Chain::from_genesis_bytes(&c.network_genesis)?;
    if !(1..=3).contains(&c.units)
        || c.budget != 100
        || c.processing_fee != 15000
        || std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "cancellation clock")?
            .as_secs()
            < manifest.closes_at_utc
    {
        return Err("cancellation terms/cutoff mismatch".into());
    }
    let required = c
        .budget
        .checked_add(c.processing_fee)
        .ok_or("cancellation total overflow")?;
    let refund = required - FEE / 4;
    let receivers: [Receiver; 6] = json(&req.receivers, 16384)?;
    let ctx = context(
        &c.conditions_hash,
        c.units,
        c.reserve,
        c.budget,
        c.processing_fee,
    );
    let mut addresses = Vec::new();
    for (i, entry) in book.entries.iter().enumerate() {
        let rec = &receivers[i];
        let address = orchard::Address::from_raw_address_bytes(
            &rec.receiver
                .as_slice()
                .try_into()
                .map_err(|_| "cancellation receiver length")?,
        )
        .into_option()
        .ok_or("cancellation receiver encoding")?;
        let actual = receiver_commitment(
            ctx,
            &entry.ticket,
            0,
            &address.g_d().to_bytes(),
            &address.pk_d().inner().to_bytes(),
            &rec.nonce,
        )?;
        if actual != entry.refund_receiver_commitment {
            return Err("cancellation receiver differs from certified refund receiver".into());
        }
        addresses.push(address);
    }
    // Only the two named operators' journals; the stopped one is never opened.
    let journals = req
        .signers
        .iter()
        .map(|i| {
            let p = req.admission.join(format!("private-node-{i}.jsonl"));
            if !p
                .try_exists()
                .map_err(|_| "cancellation journal presence")?
            {
                return Err("missing operator journal; no reset permitted".into());
            }
            Journal::open(&p, c)
        })
        .collect::<Result<Vec<_>>>()?;
    for j in &journals {
        match reserved {
            None if j.phase() == Phase::Frozen => {}
            None => return Err("operator journal already reserved another transaction".into()),
            Some(effects) => {
                j.reservation_needed(effects, true)?;
            }
        }
    }
    let raw: serde_json::Value = json(&req.notes, 1000000)?;
    if raw["notes"].as_array().is_none_or(|n| n.len() != 4) {
        return Err("cancellation requires exactly four scanner notes".into());
    }
    let mut inputs = Vec::new();
    let mut note_ids = BTreeSet::new();
    for entry in &book.entries {
        let reg = journals[0]
            .registration(&entry.ticket)
            .ok_or("cancellation registration missing")?;
        for j in &journals {
            let r = j
                .registration(&entry.ticket)
                .ok_or("operator missing accepted ticket")?;
            if j.registrations().count() != 4
                || r.note != reg.note
                || r.bid_commitment != entry.bid_commitment
                || r.receiver_commitment != entry.refund_receiver_commitment
            {
                return Err("cancellation journal/accepted book mismatch".into());
            }
        }
        if !note_ids.insert(reg.note) {
            return Err("duplicate accepted funding note".into());
        }
        let source = load_selected(&req.escrow, &req.notes, Some(reg.note))?;
        if source.chain != chain {
            return Err("escrow notes come from another network than the certified book".into());
        }
        if source.opening.coinbase
            || source.opening.amount != required
            || source.tip_height
                < source
                    .opening
                    .mined_height
                    .checked_add(1)
                    .ok_or("cancellation confirmation overflow")?
        {
            return Err("cancellation input amount/type/confirmations mismatch".into());
        }
        inputs.push(source);
    }
    let anchor = inputs[0].path.root(inputs[0].note.commitment().into());
    if inputs
        .iter()
        .any(|i| i.path.root(i.note.commitment().into()) != anchor)
    {
        return Err("cancellation native anchors differ".into());
    }
    let nullifiers: BTreeSet<_> = inputs
        .iter()
        .map(|i| i.note.nullifier(&fvk).to_bytes())
        .collect();
    if nullifiers.len() != 4 || refund * 4 + FEE != required * 4 {
        return Err("cancellation nullifier/conservation mismatch".into());
    }
    Ok(Checked {
        request: req,
        fvk,
        addresses,
        journals,
        inputs,
        anchor,
        nullifiers,
        refund,
    })
}

/// The exact four refunds, recovered with the escrow's outgoing key.
fn check_outputs<A: orchard::bundle::Authorization>(
    bundle: &orchard::Bundle<A, zcash_protocol::value::ZatBalance>,
    checked: &Checked,
    outputs: &[Output],
) -> Result<()> {
    let ovk = checked.fvk.to_ovk(Scope::External);
    let memo = memo()?;
    let mut seen = BTreeSet::new();
    if outputs.len() != 4 {
        return Err("cancellation output mapping shape".into());
    }
    for (i, output) in outputs.iter().enumerate() {
        let index = output.action_index as usize;
        if output.ticket != i || output.amount != checked.refund || !seen.insert(index) {
            return Err("cancellation output mapping".into());
        }
        let (note, address, text) = bundle
            .recover_output_with_ovk(index, &ovk)
            .ok_or("cancellation outgoing recovery")?;
        if note.value().inner() != checked.refund
            || address != checked.addresses[i]
            || text != *memo.as_array()
            || note.version() != NoteVersion::V3
            || ExtractedNoteCommitment::from(note.commitment()) != *bundle.actions()[index].cmx()
        {
            return Err("cancellation refund recipient/amount/memo mismatch".into());
        }
    }
    Ok(())
}

pub fn prepare(path: &Path) -> Result<serde_json::Value> {
    let directory = auction::directory(path)?;
    let checked = load(&directory, None)?;
    let mut rng = rand::rng();
    let tip = checked.inputs[0].tip_height;
    let height = BlockHeight::from_u32(tip.checked_add(1).ok_or("cancellation height overflow")?);
    let mut builder = Builder::new(
        Regtest,
        height,
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
            ironwood_anchor: Some(checked.anchor),
            orchard_padding: BundlePadding::DEFAULT,
            ironwood_padding: BundlePadding::DEFAULT,
        },
    )
    .with_expiry_height(height + 240);
    for source in &checked.inputs {
        builder
            .add_ironwood_spend::<Infallible>(checked.fvk.clone(), source.note, source.path.clone())
            .map_err(|_| "cancellation input build")?;
    }
    let ovk = checked.fvk.to_ovk(Scope::External);
    for address in &checked.addresses {
        builder
            .add_ironwood_output::<Infallible>(
                Some(ovk.clone()),
                *address,
                Zatoshis::from_u64(checked.refund).map_err(|_| "cancellation money range")?,
                memo()?,
            )
            .map_err(|_| "cancellation output build")?;
    }
    let parts = builder
        .build_for_pczt(&mut rng, &FeeRule::standard())
        .map_err(|_| "cancellation PCZT build")?;
    let outputs: Vec<Output> = (0..4)
        .map(|i| {
            Ok(Output {
                ticket: i,
                action_index: parts
                    .ironwood_meta
                    .output_action_index(i)
                    .ok_or("cancellation output index")? as u32,
                amount: checked.refund,
            })
        })
        .collect::<Result<_>>()?;
    let pczt = IoFinalizer::new(
        Creator::build_from_parts(parts.pczt_parts).ok_or("cancellation creator")?,
    )
    .finalize_io(&mut rng)
    .map_err(|_| "cancellation IO finalize")?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|_| "cancellation effects")?;
    let id = identity(&effects)?;
    let bundle = effects.ironwood_bundle().ok_or("cancellation bundle")?;
    if bundle.actions().len() != 4 || i64::from(bundle.value_balance()) != FEE as i64 {
        return Err("cancellation four-action shape/fee mismatch".into());
    }
    let actual: BTreeSet<_> = bundle
        .actions()
        .iter()
        .map(|a| a.nullifier().to_bytes())
        .collect();
    if actual != checked.nullifiers {
        return Err("cancellation inputs differ from accepted notes".into());
    }
    if v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
        != id.effects_digest
    {
        return Err("cancellation effects digest mismatch".into());
    }
    check_outputs(bundle, &checked, &outputs)?;
    let fvk = checked.fvk.clone();
    let pczt = Verifier::new(pczt)
        .with_ironwood::<Infallible, _>(|b| -> std::result::Result<(), OrchardError<Infallible>> {
            b.verify_cross_address_restriction()?;
            for a in b.actions() {
                a.verify_cv_net()?;
                a.spend().verify_nullifier(Some(&fvk))?;
                a.spend().verify_rk(Some(&fvk))?;
                a.output().verify_note_commitment(a.spend())?;
            }
            Ok(())
        })
        .map_err(|_| "cancellation private native fields")?
        .finish();
    write(
        &directory.join("nu7-cancel-identity.json"),
        &serde_json::to_vec(&id).map_err(|_| "cancellation identity encoding")?,
    )?;
    write(
        &directory.join("private-nu7-cancel-outputs.json"),
        &serde_json::to_vec(&outputs).map_err(|_| "cancellation mapping encoding")?,
    )?;
    write(
        &directory.join("nu7-cancel-prepared.pczt"),
        &pczt.serialize().map_err(|_| "cancellation PCZT encoding")?,
    )?;
    Ok(
        serde_json::json!({"scope":SCOPE,"txid":id.txid,"inputs":4,"refunds":4,
        "refund_each":checked.refund,"fee":FEE,"signers":checked.request.signers,
        "four_inputs_match_frozen_book":true,"refunds_to_certified_receivers":true,
        "signatures_created":false,"journals_changed":false,"broadcast":false}),
    )
}

pub fn finalize(path: &Path) -> Result<serde_json::Value> {
    let started = Instant::now();
    let directory = auction::directory(path)?;
    let encoded = auction::read(&directory.join("nu7-cancel-prepared.pczt"), 1_000_000)?;
    let pczt = Pczt::parse(&encoded).map_err(|_| "cancellation PCZT parse")?;
    if pczt
        .clone()
        .serialize()
        .map_err(|_| "cancellation PCZT serialization")?
        != encoded
    {
        return Err("cancellation PCZT noncanonical or trailing data".into());
    }
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|_| "cancellation effects")?;
    let id = identity(&effects)?;
    let saved: Identity = serde_json::from_slice(&auction::read(
        &directory.join("nu7-cancel-identity.json"),
        16384,
    )?)
    .map_err(|_| "cancellation identity schema")?;
    if saved != id {
        return Err("cancellation prepared effects changed".into());
    }
    let mut checked = load(&directory, Some(id.effects_digest))?;
    let outputs: Vec<Output> = serde_json::from_slice(&auction::read(
        &directory.join("private-nu7-cancel-outputs.json"),
        4096,
    )?)
    .map_err(|_| "cancellation mapping schema")?;
    let bundle = effects.ironwood_bundle().ok_or("cancellation bundle")?;
    let real: BTreeSet<_> = bundle
        .actions()
        .iter()
        .map(|a| a.nullifier().to_bytes())
        .collect();
    if bundle.actions().len() != 4
        || i64::from(bundle.value_balance()) != FEE as i64
        || bundle.anchor() != &checked.anchor
        || real != checked.nullifiers
    {
        return Err("cancellation shape/fee/anchor/inputs differ".into());
    }
    check_outputs(bundle, &checked, &outputs)?;
    let fvk = checked.fvk.clone();
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
                if a.spend()
                    .value()
                    .as_ref()
                    .is_none_or(|v| v.inner() != 15100)
                    || a.spend().spend_auth_sig().is_some()
                {
                    return Err(OrchardError::Custom(
                        "cancellation input value or authorization mismatch".into(),
                    ));
                }
                a.verify_cv_net()?;
                a.spend().verify_nullifier(Some(&fvk))?;
                a.spend().verify_rk(Some(&fvk))?;
                a.output().verify_note_commitment(a.spend())?;
            }
            Ok(())
        })
        .map_err(|_| "cancellation native private-field rejection")?
        .finish();
    // Reserve in both signers' journals before reading either key share.
    let mut reservation_writes = 0;
    for j in &mut checked.journals {
        reservation_writes += usize::from(j.reserve_transaction(id.effects_digest, true)?);
    }
    let (keys, dkg) = signing_keys_of(
        &checked.request.escrow,
        &checked.fvk,
        &checked.request.signers,
    )?;
    if !dkg || keys.packages.len() != 2 {
        return Err("cancellation requires the existing DKG and exactly two signers".into());
    }
    let mut rng = rand::rng();
    let mut old_rng = rand_old::rngs::OsRng;
    let proof_start = Instant::now();
    let pk = ProvingKey::build(OrchardCircuitVersion::PostNu6_3);
    let pczt = Prover::new(pczt)
        .create_ironwood_proof(&mut rng, &pk)
        .map_err(|_| "cancellation native proof")?
        .finish();
    let proof_seconds = proof_start.elapsed().as_secs_f64();
    if identity(
        &pczt
            .clone()
            .into_effects()
            .map_err(|_| "cancellation pre-sign effects")?,
    )? != id
    {
        return Err("cancellation effects changed before signing".into());
    }
    let mut spends = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<native::SignError, _>(|_, b, _| {
            for a in b.actions_mut() {
                if a.spend().spend_auth_sig().is_some() {
                    return Err(native::SignError::Custom(
                        "cancellation spend was pre-authorized".into(),
                    ));
                }
                let alpha = a
                    .spend()
                    .alpha()
                    .as_ref()
                    .ok_or_else(|| native::SignError::Custom("cancellation alpha".into()))?;
                let sig =
                    native::threshold_signature(&keys, alpha, &id.effects_digest, &mut old_rng)
                        .map_err(native::SignError::Custom)?;
                let mut changed = id.effects_digest;
                changed[0] ^= 1;
                if a.spend().rk().verify(&changed, &sig).is_ok() {
                    return Err(native::SignError::Custom(
                        "cancellation changed digest accepted".into(),
                    ));
                }
                a.apply_signature(id.effects_digest, sig)
                    .map_err(native::SignError::Apply)?;
                spends += 1;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .finish();
    if spends != 4 {
        return Err("cancellation four threshold spends required".into());
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
                    "cancellation signer changed FVK bytes".into(),
                ));
            }
            Ok(())
        })
        .map_err(|_| "cancellation signer FVK preservation")?
        .finish();
    let tx = TransactionExtractor::new(pczt)
        .extract(&mut rng)
        .map_err(|_| "cancellation extraction")?;
    if identity(&tx)? != id {
        return Err("cancellation extracted effects changed".into());
    }
    check_outputs(
        tx.ironwood_bundle().ok_or("cancellation final bundle")?,
        &checked,
        &outputs,
    )?;
    let mut bytes = Vec::new();
    tx.write(&mut bytes)
        .map_err(|_| "cancellation transaction serialization")?;
    native::verify_bytes(&bytes, BranchId::Nu7, &mut rng)?;
    write(&directory.join("cancel-nu7-transaction.bin"), &bytes)?;
    let result = serde_json::json!({"scope":"actual NU7 cancellation refund; not broadcast",
        "txid":tx.txid().to_string(),"transaction_bytes":bytes.len(),"fee":FEE,
        "refunds":4,"refund_each":checked.refund,"threshold_spends":spends,
        "signers":checked.request.signers,"dkg_source":true,"master_spending_key_read":false,
        "other_operator_key_or_journal_read":false,"one_share_rejected":true,
        "changed_digest_rejected":true,"native_proof_and_signatures_verified":true,
        "journal_reservation_writes":reservation_writes,"proof_seconds":proof_seconds,
        "seconds":started.elapsed().as_secs_f64(),"broadcast":false});
    write(
        &directory.join("cancel-construction-summary.json"),
        &serde_json::to_vec(&result).map_err(|_| "cancellation summary encoding")?,
    )?;
    Ok(result)
}
