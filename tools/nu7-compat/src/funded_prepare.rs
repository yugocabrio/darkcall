//! Certified actual-note NU7 six-output preparation. No native/application
//! proving, custody-key loading, signing, reservation or broadcast capability.
use crate::{
    Result, auction,
    interop_export::{SDK, write},
    regtest_transfer::{Regtest, load_selected},
    regtest_wallet,
};
use darkcall_input_tool::{context, receiver_commitment};
use darkcall_wallet_protocol::{
    final_book::{Certificate, Manifest},
    journal::Journal,
};
use group::GroupEncoding;
use orchard::{
    keys::Scope,
    note::{ExtractedNoteCommitment, NoteVersion},
};
use pczt::roles::{
    creator::Creator,
    io_finalizer::IoFinalizer,
    verifier::{OrchardError, Verifier},
};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    convert::Infallible,
    path::{Path, PathBuf},
};
use zcash_primitives::transaction::{
    TxVersion,
    builder::{BuildConfig, Builder, BundlePadding},
    fees::zip317::FeeRule,
    sighash::SignableInput,
    sighash_v6::v6_signature_hash,
    txid::{TxIdDigester, to_txid},
};
use zcash_protocol::{
    consensus::{BlockHeight, BranchId},
    value::Zatoshis,
};

pub const SCOPE: &str = "actual isolated NU7 six-output auction preparation; no authorization";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    format: u32,
    scope: String,
    sdk: String,
    branch: String,
    pub(super) escrow: PathBuf,
    pub(super) notes: PathBuf,
    pub(super) receivers: PathBuf,
    pub(super) admission: PathBuf,
    pub(super) amounts: [u64; 6],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receiver {
    receiver: Vec<u8>,
    nonce: [u8; 32],
}
fn json<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    auction::directory(path.parent().ok_or("funded input parent")?)?;
    serde_json::from_slice(&auction::read(path, limit)?)
        .map_err(|_| "funded strict input schema".into())
}
pub(super) struct Validated {
    pub request: Request,
    pub fvk: orchard::keys::FullViewingKey,
    pub addresses: Vec<orchard::Address>,
    pub memos: [zcash_protocol::memo::MemoBytes; 6],
    pub journals: Vec<Journal>,
    pub inputs: Vec<crate::regtest_transfer::Source>,
    pub tip: u32,
    pub anchor: orchard::Anchor,
    pub nullifiers: BTreeSet<[u8; 32]>,
}
pub(super) fn load(path: &Path, reserved: Option<[u8; 32]>) -> Result<Validated> {
    let directory = auction::directory(path)?;
    let req: Request = json(&directory.join("nu7-funded-request.json"), 32768)?;
    if req.format != 1 || req.scope != SCOPE || req.sdk != SDK || req.branch != "77190ad9" {
        return Err("funded NU7 request scope/pin mismatch".into());
    }
    let fvk = regtest_wallet::viewing_key(&req.escrow)?;
    if req.escrow.join("private-spending-key.bin").exists() {
        return Err("funded NU7 preparation requires DKG view-only escrow".into());
    }
    let manifest: Manifest = json(&req.admission.join("manifest.json"), 16384)?;
    let certificate: Certificate =
        json(&req.admission.join("final-book-certificate.json"), 131072)?;
    certificate.verify(&manifest)?;
    let c = &manifest.context;
    let book = &certificate.endorsements[0].book;
    let chain = crate::network::Chain::from_genesis_bytes(&c.network_genesis)?;
    // One to three units: the same four tickets and six outputs at any count.
    if !(1..=3).contains(&c.units)
        || c.budget != 100
        || c.processing_fee != 15000
        || std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "funded clock")?
            .as_secs()
            < manifest.closes_at_utc
    {
        return Err("funded prototype terms/genesis/cutoff mismatch".into());
    }
    let receipt_book: crate::regtest_deposits::Book =
        json(&directory.join("receipt-book.json"), 16384)?;
    let receipt_request: serde_json::Value =
        json(&directory.join("nu7-auction-request.json"), 4096)?;
    if receipt_book.condition_digest != c.conditions_hash
        || receipt_book.ids != book.entries.each_ref().map(|e| e.ticket)
        || darkcall_input_tool::decimal_field_bytes(&receipt_book.root)? != book.accepted_root
        || (
            receipt_book.units,
            receipt_book.reserve,
            receipt_book.budget,
            receipt_book.processing_fee,
        ) != (c.units, c.reserve, c.budget, c.processing_fee)
        || receipt_request["scope"] != SCOPE
        || receipt_request["amounts"] != serde_json::json!(req.amounts)
    {
        return Err("funded receipt book/request differs from accepted auction".into());
    }
    let memos = auction::checked_receipt_memos(&directory, false)?;
    let receivers: [Receiver; 6] = json(&req.receivers, 16384)?;
    let ctx = context(
        &c.conditions_hash,
        c.units,
        c.reserve,
        c.budget,
        c.processing_fee,
    );
    let mut addresses = Vec::new();
    for (i, rec) in receivers.iter().enumerate() {
        let address = orchard::Address::from_raw_address_bytes(
            &rec.receiver
                .as_slice()
                .try_into()
                .map_err(|_| "funded receiver length")?,
        )
        .into_option()
        .ok_or("funded receiver encoding")?;
        let ticket = if i < 4 {
            book.entries[i].ticket
        } else {
            [0; 32]
        };
        let role = if i < 4 {
            0
        } else if i == 4 {
            1
        } else {
            2
        };
        let actual = receiver_commitment(
            ctx,
            &ticket,
            role,
            &address.g_d().to_bytes(),
            &address.pk_d().inner().to_bytes(),
            &rec.nonce,
        )?;
        let expected = if i < 4 {
            book.entries[i].refund_receiver_commitment
        } else if i == 4 {
            manifest.seller_receiver_commitment
        } else {
            manifest.operator_receiver_commitment
        };
        if actual != expected {
            return Err("funded receiver differs from certified client intent".into());
        }
        addresses.push(address);
    }
    let journals = (0..3)
        .map(|i| {
            let p = req.admission.join(format!("private-node-{i}.jsonl"));
            if !p.try_exists().map_err(|_| "funded journal presence")? {
                return Err("missing funded journal; no reset permitted".into());
            }
            Journal::open(&p, c)
        })
        .collect::<Result<Vec<_>>>()?;
    let raw: serde_json::Value = json(&req.notes, 1000000)?;
    if raw["notes"].as_array().is_none_or(|n| n.len() != 4) {
        return Err("funded preparation requires exactly four scanner notes".into());
    }
    let mut inputs = Vec::new();
    let mut note_ids = BTreeSet::new();
    let required = c
        .budget
        .checked_add(c.processing_fee)
        .ok_or("funded total overflow")?;
    for entry in &book.entries {
        let reg = journals[0]
            .registration(&entry.ticket)
            .ok_or("funded registration missing")?;
        for j in &journals {
            let r = j
                .registration(&entry.ticket)
                .ok_or("operator missing funded ticket")?;
            if !(j.phase() == darkcall_auction_core::funding::Phase::Frozen
                || (j.phase() == darkcall_auction_core::funding::Phase::Prepared
                    && reserved.is_some_and(|e| j.reserved_effects() == Some(e))))
                || j.registrations().count() != 4
                || r.note != reg.note
                || r.bid_commitment != entry.bid_commitment
                || r.receiver_commitment != entry.refund_receiver_commitment
            {
                return Err("funded journal/accepted book mismatch".into());
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
                    .ok_or("funded confirmation overflow")?
        {
            return Err("funded actual input amount/type/confirmations mismatch".into());
        }
        inputs.push(source);
    }
    let tip = inputs[0].tip_height;
    let anchor = inputs[0].path.root(inputs[0].note.commitment().into());
    let expected_nullifiers: BTreeSet<_> = inputs
        .iter()
        .map(|i| i.note.nullifier(&fvk).to_bytes())
        .collect();
    if expected_nullifiers.len() != 4
        || req
            .amounts
            .iter()
            .try_fold(30000u64, |a, b| a.checked_add(*b))
            != required.checked_mul(4)
    {
        return Err("funded nullifier/conservation mismatch".into());
    }
    if inputs
        .iter()
        .any(|i| i.path.root(i.note.commitment().into()) != anchor)
    {
        return Err("funded native anchors differ".into());
    }
    Ok(Validated {
        request: req,
        fvk,
        addresses,
        memos,
        journals,
        inputs,
        tip,
        anchor,
        nullifiers: expected_nullifiers,
    })
}
pub fn prepare(path: &Path) -> Result<serde_json::Value> {
    let directory = auction::directory(path)?;
    let Validated {
        request: req,
        fvk,
        addresses,
        memos,
        journals: _journals,
        inputs,
        tip,
        anchor,
        nullifiers: expected_nullifiers,
    } = load(&directory, None)?;
    let mut rng = rand::rng();
    let height = BlockHeight::from_u32(tip.checked_add(1).ok_or("funded height overflow")?);
    let mut builder = Builder::new(
        Regtest,
        height,
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
            ironwood_anchor: Some(anchor),
            orchard_padding: BundlePadding::DEFAULT,
            ironwood_padding: BundlePadding::DEFAULT,
        },
    )
    .with_expiry_height(height + 240);
    for source in inputs {
        if source.path.root(source.note.commitment().into()) != anchor {
            return Err("funded native anchors differ".into());
        }
        builder
            .add_ironwood_spend::<Infallible>(fvk.clone(), source.note, source.path)
            .map_err(|_| "funded native input build")?;
    }
    let ovk = fvk.to_ovk(Scope::External);
    for (i, address) in addresses.iter().enumerate() {
        builder
            .add_ironwood_output::<Infallible>(
                Some(ovk.clone()),
                *address,
                Zatoshis::from_u64(req.amounts[i]).map_err(|_| "funded output money range")?,
                memos[i].clone(),
            )
            .map_err(|_| "funded native output build")?;
    }
    let parts = builder
        .build_for_pczt(&mut rng, &FeeRule::standard())
        .map_err(|_| "funded NU7 PCZT build")?;
    let indices: [usize; 6] =
        std::array::from_fn(|i| parts.ironwood_meta.output_action_index(i).unwrap());
    let pczt =
        IoFinalizer::new(Creator::build_from_parts(parts.pczt_parts).ok_or("funded NU7 creator")?)
            .finalize_io(&mut rng)
            .map_err(|_| "funded NU7 IO finalize")?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|_| "funded NU7 effects")?;
    if effects.version() != TxVersion::V6
        || effects.consensus_branch_id() != BranchId::Nu7
        || effects.transparent_bundle().is_some()
        || effects.sapling_bundle().is_some()
        || effects.orchard_bundle().is_some()
    {
        return Err("funded NU7 effects branch/pool mismatch".into());
    }
    let bundle = effects.ironwood_bundle().ok_or("funded native bundle")?;
    if bundle.actions().len() != 6 || i64::from(bundle.value_balance()) != 30000 {
        return Err("funded six-output shape/fee mismatch".into());
    }
    let actual_nullifiers: BTreeSet<_> = bundle
        .actions()
        .iter()
        .filter_map(|a| {
            let n = a.nullifier().to_bytes();
            expected_nullifiers.contains(&n).then_some(n)
        })
        .collect();
    if actual_nullifiers != expected_nullifiers {
        return Err("funded native inputs differ from accepted notes".into());
    }
    let native_actions: [_; 6] = bundle
        .actions()
        .iter()
        .map(|a| a.cmx().to_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "funded native action shape")?;
    let txid = to_txid(
        effects.version(),
        effects.consensus_branch_id(),
        &effects.digest(TxIdDigester),
    );
    let digest = *txid.as_ref();
    if v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
        != digest
    {
        return Err("funded native effects digest mismatch".into());
    }
    let mut openings = Vec::new();
    for (i, index) in indices.iter().enumerate() {
        let (note, address, memo) = bundle
            .recover_output_with_ovk(*index, &ovk)
            .ok_or("funded outgoing recovery")?;
        if note.value().inner() != req.amounts[i]
            || address != addresses[i]
            || memo != *memos[i].as_array()
            || note.version() != NoteVersion::V3
            || ExtractedNoteCommitment::from(note.commitment()) != *bundle.actions()[*index].cmx()
        {
            return Err("funded actual native recipient/amount/memo mismatch".into());
        }
        openings.push(serde_json::json!({"action_index":*index as u32,"recipient":address.to_raw_address_bytes().to_vec(),
            "amount":note.value().inner(),"rho":note.rho().to_bytes(),"rseed":note.rseed().as_bytes(),"cmx":native_actions[*index]}));
    }
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
        .map_err(|_| "funded NU7 private native fields")?
        .finish();
    let identity = serde_json::json!({"format":1,"scope":SCOPE,"sdk":SDK,"branch":"77190ad9",
        "txid":txid.to_string(),"effects_digest":digest,"native_actions":native_actions});
    write(
        &directory.join("nu7-native-identity.json"),
        &serde_json::to_vec(&identity).unwrap(),
    )?;
    write(&directory.join("private-nu7-auction-outputs.json"),&serde_json::to_vec(&serde_json::json!({
        "format":1,"scope":SCOPE,"sdk":SDK,"branch":"77190ad9","txid":txid.to_string(),"outputs":openings})).unwrap())?;
    write(
        &directory.join("nu7-prepared.pczt"),
        &pczt.serialize().map_err(|_| "funded PCZT encoding")?,
    )?;
    Ok(
        serde_json::json!({"scope":SCOPE,"four_actual_inputs_match_frozen_book":true,
        "outputs":6,"refund_receipt_memos":4,"fee":30000,"actual_outgoing_recoveries":6,
        "recipient_viewing_or_spending_keys_read":false,"custody_key_packages_read":false,
        "proving_keys_created":false,"application_binding_proofs_generated":false,
        "native_proof_or_real_threshold_signatures_created":false,"funding_reservations_changed":false,"broadcast":false,
        "chain_provenance":"caller must verify same canonical scanner snapshot before preparation"}),
    )
}
