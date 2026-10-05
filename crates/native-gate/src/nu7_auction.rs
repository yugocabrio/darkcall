//! Auction proof adapter for the separately pinned NU7 SDK. All cross-version
//! values are canonical bytes; this working SDK never parses a NU7 transaction.
use super::*;
use crate::integration::checked;
use crate::regtest_spend::{read_private, write_private};
use darkcall_bn254_emulation::poseidon::{digest_words, pack_row, reference};
use darkcall_output_binding::poseidon_rows::{
    HASH_RULE, LayoutProver, RowLayout, binding_from_note, read_layout, verify_set_for_layout,
};
use darkcall_settlement_types::{NativeBatchStatement, NativeContext, link_four_bid_statements};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const SDK: &str = "9c5705f56517c040c50f96d29f1ff15a9383f3e0";
const SCOPE: &str = "synthetic NU7 six-output auction; no chain funding";
const FUNDED_PREPARATION: &str =
    "actual isolated NU7 six-output auction preparation; no authorization";
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Identity {
    format: u32,
    scope: String,
    sdk: String,
    branch: String,
    txid: String,
    effects_digest: [u8; 32],
    native_actions: [[u8; 32]; 6],
}
impl Identity {
    fn validate(&self) -> Result<()> {
        self.validate_scope(SCOPE)
    }
    fn validate_scope(&self, scope: &str) -> Result<()> {
        // Shielded-only v6 sighash equals the raw txid. NU7 adapter rejects
        // transparent and other-pool bundles before producing this identity.
        let mut reversed = self.effects_digest;
        reversed.reverse();
        if self.format != 1
            || self.scope != scope
            || self.sdk != SDK
            || self.branch != "77190ad9"
            || self.txid != hex::encode(reversed)
        {
            return Err("NU7 application native identity mismatch".into());
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Opening {
    action_index: u32,
    recipient: Vec<u8>,
    amount: u64,
    rho: [u8; 32],
    rseed: [u8; 32],
    cmx: [u8; 32],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Openings {
    format: u32,
    scope: String,
    sdk: String,
    branch: String,
    txid: String,
    outputs: Vec<Opening>,
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}
fn sdk() -> PathBuf {
    root().join(".deps/nu7-compat-target/release/darkcall-nu7-compat")
}
fn local(path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize().map_err(|_| "NU7 directory missing")?;
    if !path.starts_with(
        root()
            .join(".local")
            .canonicalize()
            .map_err(|_| "local directory")?,
    ) || !path.is_dir()
    {
        return Err("NU7 adapter requires ignored synthetic directories".into());
    }
    Ok(path)
}
fn json<T: serde::de::DeserializeOwned>(directory: &Path, name: &str, limit: u64) -> Result<T> {
    serde_json::from_slice(&read_private(&directory.join(name), limit)?)
        .map_err(|_| "strict NU7 application artifact schema".into())
}
fn checked_sdk(mode: &str, directory: &Path) -> Result<serde_json::Value> {
    let log = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(format!("{mode}.log")))
        .map_err(|_| "new SDK log")?;
    let output = Command::new(sdk())
        .arg(mode)
        .arg(directory)
        .stderr(Stdio::from(log))
        .output()
        .map_err(|_| "new SDK process launch")?;
    if !output.status.success() {
        return Err("NU7 adapter failed; inspect ignored SDK log".into());
    }
    if output.stdout.len() > 65536 {
        return Err("NU7 SDK summary bound".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "new SDK summary encoding".into())
}

/// Exact private row/note reconstruction for a local warmed prover. No keys,
/// bids, PCZT or signing capability are loaded by this path.
pub(crate) fn worker_bindings(
    path: &Path,
) -> Result<(
    NativeBatchStatement,
    Vec<darkcall_output_binding::sha::Binding>,
)> {
    let directory = local(path)?;
    if read_layout(&directory).map_err(|_| "worker native layout")? != RowLayout::Shared18 {
        return Err("worker requires the fixed shared18 layout".into());
    }
    let identity: Identity = json(&directory, "nu7-native-identity.json", 16384)?;
    identity.validate()?;
    let data: Openings = json(&directory, "private-nu7-auction-outputs.json", 32768)?;
    let statement: NativeBatchStatement = json(&directory, "native-public.json", 32768)?;
    let encoded: Vec<Vec<u8>> = json(&directory, "native-private-rows.json", 32768)?;
    if data.format != 1
        || data.scope != SCOPE
        || data.sdk != SDK
        || data.branch != identity.branch
        || data.txid != identity.txid
        || data.outputs.len() != 6
        || encoded.len() != 6
        || statement.native_actions != identity.native_actions
    {
        return Err("worker exact native identity/shape mismatch".into());
    }
    link_four_bid_statements(
        &statement.row_hashes,
        &statement.bindings(),
        &identity.native_actions,
    )
    .map_err(|_| "worker action mapping mismatch")?;
    let mut bindings = Vec::new();
    for (i, opening) in data.outputs.iter().enumerate() {
        let row = Row::decode(&encoded[i]).map_err(|_| "worker canonical row")?;
        if row.action_index != opening.action_index
            || row.action_index != statement.action_indices[i]
            || opening.cmx != statement.cmxs[i]
            || digest_words(&reference(&pack_row(&row.encode()))) != statement.row_hashes[i]
        {
            return Err("worker private row/native statement mismatch".into());
        }
        let address = orchard::Address::from_raw_address_bytes(
            opening
                .recipient
                .as_slice()
                .try_into()
                .map_err(|_| "worker recipient length")?,
        )
        .into_option()
        .ok_or("worker recipient encoding")?;
        let rho = Rho::from_bytes(&opening.rho)
            .into_option()
            .ok_or("worker rho")?;
        let seed = RandomSeed::from_bytes(opening.rseed, &rho)
            .into_option()
            .ok_or("worker seed")?;
        let note = Note::from_parts(
            address,
            NoteValue::from_raw(opening.amount),
            rho,
            seed,
            NoteVersion::V3,
        )
        .into_option()
        .ok_or("worker note")?;
        if ExtractedNoteCommitment::from(note.commitment()).to_bytes() != opening.cmx {
            return Err("worker native commitment mismatch".into());
        }
        bindings.push(
            binding_from_note(&row, &note).map_err(|_| "worker row recipient/amount mismatch")?,
        );
    }
    Ok((statement, bindings))
}

pub fn run(book: &Path, name: &str, prove: bool) -> Result<serde_json::Value> {
    run_with_memos(book, name, prove, false)
}

/// Actual certified funding -> same six-output receipt preparation. Does not
/// weaken the existing synthetic verifier's scope or authorize a funded spend.
pub fn prepare_funded(
    escrow: &Path,
    funding: &Path,
    book: &Path,
    receivers: &Path,
    admission: &Path,
    name: &str,
    prove: bool,
) -> Result<serde_json::Value> {
    use darkcall_wallet_bridge::final_book::{Certificate, Manifest};
    let started = Instant::now();
    let escrow = local(escrow)?;
    let admission = local(admission)?;
    let funding = local(funding.parent().ok_or("funding parent")?)?
        .join(funding.file_name().ok_or("funding file")?);
    let receivers = local(receivers.parent().ok_or("receiver parent")?)?
        .join(receivers.file_name().ok_or("receiver file")?);
    let manifest: Manifest = json(&admission, "manifest.json", 16384)?;
    let certificate: Certificate = json(&admission, "final-book-certificate.json", 131072)?;
    certificate.verify(&manifest)?;
    let accepted = &certificate.endorsements[0].book;
    let mut integration = Integration::new_with_receipts(book, name, RowLayout::Shared18, true)?;
    if integration.context.conditions != manifest.context.conditions_hash
        || integration.context.accepted_root != accepted.accepted_root
        || integration.context.ticket_ids != accepted.entries.each_ref().map(|e| e.ticket)
    {
        return Err("funded MPC input book differs from certified admission".into());
    }
    integration.funding_is_synthetic = false;
    integration.receiver_openings = Some(receivers.clone());
    let directory = &integration.directory;
    write_private(
        &directory.join("final-book-certificate.json"),
        &serde_json::to_vec(&certificate).map_err(|_| "funded certificate encoding")?,
    )?;
    write_private(
        &directory.join("nu7-auction-request.json"),
        &serde_json::to_vec(&serde_json::json!({
        "format":1,"scope":FUNDED_PREPARATION,"sdk":SDK,"branch":"77190ad9",
        "amounts":integration.delivery.amounts,"receipt_memos":true}))
        .unwrap(),
    )?;
    write_private(&directory.join("nu7-funded-request.json"),&serde_json::to_vec(&serde_json::json!({
        "format":1,"scope":FUNDED_PREPARATION,"sdk":SDK,"branch":"77190ad9",
        "escrow":escrow,"notes":funding,"admission":admission,"receivers":receivers,"amounts":integration.delivery.amounts})).unwrap())?;
    let prepared = checked_sdk("prepare-funded-auction", directory)?;
    let identity: Identity = json(directory, "nu7-native-identity.json", 16384)?;
    let data: Openings = json(directory, "private-nu7-auction-outputs.json", 32768)?;
    let mut reversed = identity.effects_digest;
    reversed.reverse();
    if identity.format != 1
        || identity.scope != FUNDED_PREPARATION
        || identity.sdk != SDK
        || identity.branch != "77190ad9"
        || identity.txid != hex::encode(reversed)
        || data.format != 1
        || data.scope != FUNDED_PREPARATION
        || data.sdk != SDK
        || data.branch != identity.branch
        || data.txid != identity.txid
        || data.outputs.len() != 6
    {
        return Err("funded NU7 native identity mismatch".into());
    }
    let mut rows = Vec::new();
    let mut notes = Vec::new();
    let mut used = [false; 6];
    for (i, opening) in data.outputs.iter().enumerate() {
        let index = opening.action_index as usize;
        if index >= 6
            || used[index]
            || opening.cmx != identity.native_actions[index]
            || opening.amount != integration.delivery.amounts[i]
        {
            return Err("funded NU7 output amount/mapping mismatch".into());
        }
        used[index] = true;
        let address = orchard::Address::from_raw_address_bytes(
            &opening
                .recipient
                .as_slice()
                .try_into()
                .map_err(|_| "funded receiver bytes")?,
        )
        .into_option()
        .ok_or("funded receiver encoding")?;
        let rho = Rho::from_bytes(&opening.rho)
            .into_option()
            .ok_or("funded rho")?;
        let seed = RandomSeed::from_bytes(opening.rseed, &rho)
            .into_option()
            .ok_or("funded seed")?;
        let note = Note::from_parts(
            address,
            NoteValue::from_raw(opening.amount),
            rho,
            seed,
            NoteVersion::V3,
        )
        .into_option()
        .ok_or("funded V3 note")?;
        if ExtractedNoteCommitment::from(note.commitment()).to_bytes() != opening.cmx {
            return Err("funded reconstructed note differs from native commitment".into());
        }
        let mut salt = [0; 32];
        OsRng.fill_bytes(&mut salt);
        let row = Row {
            conditions: integration.context.conditions,
            accepted_root: integration.context.accepted_root,
            ticket: if i < 4 {
                integration.context.ticket_ids[i]
            } else {
                [0; 32]
            },
            role: if i < 4 {
                Role::Refund
            } else if i == 4 {
                Role::Seller
            } else {
                Role::Operator
            },
            action_index: opening.action_index,
            gd: address.g_d().to_bytes(),
            pkd: address.pk_d().inner().to_bytes(),
            amount: opening.amount,
            salt,
        };
        binding_from_note(&row, &note)
            .map_err(|_| "funded canonical row recipient/amount mismatch")?;
        rows.push(row);
        notes.push(note);
    }
    let notes: [_; 6] = notes.try_into().map_err(|_| "funded notes count")?;
    let rows: [_; 6] = rows.try_into().map_err(|_| "funded rows count")?;
    let statement = NativeBatchStatement {
        row_hashes: rows
            .each_ref()
            .map(|r| digest_words(&reference(&pack_row(&r.encode())))),
        action_indices: rows.each_ref().map(|r| r.action_index),
        cmxs: std::array::from_fn(|i| identity.native_actions[rows[i].action_index as usize]),
        native_actions: identity.native_actions,
    };
    link_four_bid_statements(
        &statement.row_hashes,
        &statement.bindings(),
        &identity.native_actions,
    )
    .map_err(|_| "funded canonical native action mapping")?;
    write_private(
        &directory.join("private-nu7-funded-row-checkpoint.json"),
        &serde_json::to_vec(&rows.each_ref().map(|r| r.encode().to_vec())).unwrap(),
    )?;
    write_private(
        &directory.join("nu7-funded-expected-native-public.json"),
        &serde_json::to_vec(&statement).unwrap(),
    )?;
    if prove {
        let approval = integration.prove_and_check(
            &rows,
            &notes,
            identity.native_actions,
            identity.effects_digest,
        )?;
        approval.check(&identity.native_actions, &identity.effects_digest)?;
        let encoded = read_private(&approval.mpc.join("public-input0"), 232 * 32)?;
        certificate.verify_statement(&encoded, &manifest)?;
        let native = checked_sdk("finalize-funded-auction", directory)?;
        if native["txid"] != identity.txid
            || native["threshold_spends"] != 4
            || native["dkg_source"] != true
            || native["master_spending_key_read"] != false
        {
            return Err("funded final SDK identity/setup mismatch".into());
        }
        return Ok(
            serde_json::json!({"scope":"actual isolated NU7 certified six-output auction construction; not broadcast",
            "prepared":prepared,"native":native,"same_pczt_transaction":true,"blinded_v2_mpc_delivery_verified":true,
            "accepted_root_and_recipients_proof_linked":true,"price":integration.delivery.price,"sold":integration.delivery.sold,
            "maximum_bids_read":false,"seconds":started.elapsed().as_secs_f64(),"broadcast":false}),
        );
    }
    Ok(
        serde_json::json!({"scope":FUNDED_PREPARATION,"prepared":prepared,"price":integration.delivery.price,
        "sold":integration.delivery.sold,"maximum_bids_read":false,"blinded_v2_mpc_delivery_verified":true,
        "normal_cpp_winner_proof_verified":true,"six_canonical_native_row_openings_match":true,
        "four_receipt_memos_in_same_six_output_pczt":true,"accepted_root_authenticated":true,
        "application_binding_proofs_generated":false,"native_proof_or_real_threshold_signatures_created":false,
        "funding_reserved_or_broadcast":false,"seconds":started.elapsed().as_secs_f64()}),
    )
}
pub fn run_with_memos(
    book: &Path,
    name: &str,
    prove: bool,
    receipts: bool,
) -> Result<serde_json::Value> {
    run_inner(book, name, prove, receipts, None)
}

/// Resume one unchanged unsigned funded transaction after a resource stop.
/// Original artifacts/journals remain intact; changed effects are never chosen.
pub fn resume_funded(book: &Path, source: &Path, name: &str) -> Result<serde_json::Value> {
    use darkcall_wallet_bridge::final_book::{Certificate, Manifest};
    let started = Instant::now();
    let source = local(source)?;
    let book = local(book)?;
    if name.is_empty()
        || name.len() > 50
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("fresh funded resume name required".into());
    }
    let directory = root().join(".local").join(name);
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|_| "fresh funded resume directory")?;
    for (file, limit) in [
        ("nu7-funded-request.json", 32768),
        ("nu7-auction-request.json", 4096),
        ("nu7-native-identity.json", 16384),
        ("nu7-prepared.pczt", 1_000_000),
        ("private-nu7-auction-outputs.json", 32768),
        ("context.json", 16384),
        ("receipt-book.json", 16384),
        ("final-book-certificate.json", 131072),
        ("native-layout.json", 4096),
        ("ticket-0.memo", 512),
        ("ticket-1.memo", 512),
        ("ticket-2.memo", 512),
        ("ticket-3.memo", 512),
    ] {
        write_private(
            &directory.join(file),
            &read_private(&source.join(file), limit)?,
        )?;
    }
    fs::DirBuilder::new()
        .mode(0o700)
        .create(directory.join("receipt-proof-public"))
        .map_err(|_| "funded resume receipt directory")?;
    for file in [
        "vk",
        "proof.0",
        "public-input0",
        "tampered-price",
        "tampered-root",
        "tampered-cipher",
        "tampered-winner",
    ] {
        write_private(
            &directory.join("receipt-proof-public").join(file),
            &read_private(&source.join("receipt-proof-public").join(file), 1_048_576)?,
        )?;
    }
    let request: serde_json::Value = json(&directory, "nu7-funded-request.json", 32768)?;
    let admission = local(Path::new(
        request["admission"]
            .as_str()
            .ok_or("funded admission path")?,
    ))?;
    let manifest: Manifest = json(&admission, "manifest.json", 16384)?;
    let certificate: Certificate = json(&directory, "final-book-certificate.json", 131072)?;
    certificate.verify(&manifest)?;
    let prepared = checked_sdk("check-funded-prepared", &directory)?;
    let identity: Identity = json(&directory, "nu7-native-identity.json", 16384)?;
    identity.validate_scope(FUNDED_PREPARATION)?;
    if prepared["txid"] != identity.txid {
        return Err("funded resume native identity differs".into());
    }
    let context: NativeContext = json(&directory, "context.json", 16384)?;
    if context.conditions != manifest.context.conditions_hash
        || context.accepted_root != certificate.endorsements[0].book.accepted_root
        || context.ticket_ids
            != certificate.endorsements[0]
                .book
                .entries
                .each_ref()
                .map(|e| e.ticket)
    {
        return Err("funded resume context differs from certified acceptance".into());
    }
    let result: serde_json::Value = json(&source, "delivery-result.json", 4096)?;
    let run = local(Path::new(
        result["run"].as_str().ok_or("funded delivery path")?,
    ))?;
    checked(
        Command::new("python3")
            .arg(root().join("scripts/verify_external.py"))
            .arg(&run)
            .args([
                "--circuit",
                "delivery_flags4",
                "--output-label",
                &format!("funded-resume-{}", std::process::id()),
            ]),
        &directory.join("resumed-delivery-proof.log"),
    )?;
    checked(
        Command::new(root().join("target/release/darkcall-input-tool"))
            .arg("flags-result")
            .arg(book.join("book.json"))
            .arg(run.join("plan/masks.json"))
            .arg(run.join("plan/winner-masks.json"))
            .arg(run.join("public-input0"))
            .arg(directory.join("private-resumed-delivery.json")),
        &directory.join("resumed-delivery-opening.log"),
    )?;
    let delivery = crate::integration::Delivery::from_opening(
        &json(&directory, "private-resumed-delivery.json", 4096)?,
        run,
        true,
    )?;
    if serde_json::json!(delivery.amounts) != request["amounts"] {
        return Err("resumed private delivery differs from actual outputs".into());
    }
    let encoded: [Vec<u8>; 6] = json(&source, "native-private-rows.json", 32768)?;
    let data: Openings = json(&directory, "private-nu7-auction-outputs.json", 32768)?;
    if data.scope != FUNDED_PREPARATION || data.txid != identity.txid || data.outputs.len() != 6 {
        return Err("funded resume opening identity differs".into());
    }
    let mut rows = Vec::new();
    let mut notes = Vec::new();
    for (i, opening) in data.outputs.iter().enumerate() {
        let row = Row::decode(&encoded[i]).map_err(|_| "funded resume row codec")?;
        if row.conditions != context.conditions
            || row.accepted_root != context.accepted_root
            || row.ticket
                != if i < 4 {
                    context.ticket_ids[i]
                } else {
                    [0; 32]
                }
            || row.role
                != if i < 4 {
                    Role::Refund
                } else if i == 4 {
                    Role::Seller
                } else {
                    Role::Operator
                }
            || row.action_index != opening.action_index
            || row.action_index >= 6
            || opening.cmx != identity.native_actions[row.action_index as usize]
            || opening.amount != delivery.amounts[i]
        {
            return Err("funded resume canonical row/context/action/amount differs".into());
        }
        let address = orchard::Address::from_raw_address_bytes(
            &opening
                .recipient
                .as_slice()
                .try_into()
                .map_err(|_| "funded resume receiver length")?,
        )
        .into_option()
        .ok_or("funded resume receiver")?;
        let rho = Rho::from_bytes(&opening.rho)
            .into_option()
            .ok_or("funded resume rho")?;
        let seed = RandomSeed::from_bytes(opening.rseed, &rho)
            .into_option()
            .ok_or("funded resume seed")?;
        let note = Note::from_parts(
            address,
            NoteValue::from_raw(opening.amount),
            rho,
            seed,
            NoteVersion::V3,
        )
        .into_option()
        .ok_or("funded resume V3 note")?;
        if ExtractedNoteCommitment::from(note.commitment()).to_bytes() != opening.cmx {
            return Err("funded resume note commitment differs".into());
        }
        binding_from_note(&row, &note)
            .map_err(|_| "funded resume canonical recipient/amount differs")?;
        rows.push(row);
        notes.push(note);
    }
    let rows: [_; 6] = rows.try_into().map_err(|_| "funded resume rows")?;
    let notes: [_; 6] = notes.try_into().map_err(|_| "funded resume notes")?;
    let integration = Integration {
        root: root(),
        book,
        directory: directory.clone(),
        context,
        delivery,
        receiver_openings: Some(PathBuf::from(
            request["receivers"]
                .as_str()
                .ok_or("funded receiver openings path")?,
        )),
        funding_is_synthetic: false,
        native_proof_source: Some(source.clone()),
        layout: RowLayout::Shared18,
    };
    let approval = integration.prove_and_check(
        &rows,
        &notes,
        identity.native_actions,
        identity.effects_digest,
    )?;
    approval.check(&identity.native_actions, &identity.effects_digest)?;
    certificate.verify_statement(
        &read_private(&approval.mpc.join("public-input0"), 232 * 32)?,
        &manifest,
    )?;
    let native = checked_sdk("finalize-funded-auction", &directory)?;
    if native["txid"] != identity.txid
        || native["threshold_spends"] != 4
        || native["dkg_source"] != true
    {
        return Err("funded resumed final native identity/setup differs".into());
    }
    Ok(
        serde_json::json!({"scope":"actual isolated NU7 exact prepared memo auction construction; not broadcast",
        "prepared":prepared,"native":native,"same_pczt_transaction":true,"prepared_pczt_reused":true,
        "source_pczt_and_rows_unchanged":true,"accepted_root_and_recipients_proof_linked":true,
        "blinded_v2_mpc_delivery_verified":true,"maximum_bids_read":false,
        "price":integration.delivery.price,"sold":integration.delivery.sold,
        "seconds":started.elapsed().as_secs_f64(),"broadcast":false}),
    )
}

/// Same six-output statement/signing checks, with a caller-owned fixed PK.
/// This opt-in service is synthetic only; independent SDK application replay is
/// retained before signing and never trusts a worker's success message alone.
pub(crate) fn run_warm(
    book: &Path,
    name: &str,
    prover: &LayoutProver,
    workers: usize,
) -> Result<serde_json::Value> {
    crate::row_worker::capacity(workers)?;
    run_inner(book, name, true, true, Some((prover, workers)))
}

fn run_inner(
    book: &Path,
    name: &str,
    prove: bool,
    receipts: bool,
    warm: Option<(&LayoutProver, usize)>,
) -> Result<serde_json::Value> {
    let start = Instant::now();
    let integration = Integration::new_with_receipts(book, name, RowLayout::Shared18, receipts)?;
    if warm.is_some() {
        crate::row_worker::clearing(name, integration.delivery.price, integration.delivery.sold)?;
    }
    let directory = &integration.directory;
    write_private(&directory.join("nu7-auction-request.json"), &serde_json::to_vec(&serde_json::json!({
        "format":1,"scope":SCOPE,"sdk":SDK,"branch":"77190ad9","amounts":integration.delivery.amounts,"receipt_memos":receipts
    })).map_err(|_| "NU7 amount request encoding")?)?;
    let prepared = checked_sdk("prepare-auction", directory)?;
    let identity: Identity = json(directory, "nu7-native-identity.json", 16384)?;
    identity.validate()?;
    let data: Openings = json(directory, "private-nu7-auction-outputs.json", 32768)?;
    if data.format != 1
        || data.scope != SCOPE
        || data.sdk != SDK
        || data.branch != identity.branch
        || data.txid != identity.txid
        || data.outputs.len() != 6
    {
        return Err("NU7 auction openings identity mismatch".into());
    }
    let mut rows = Vec::new();
    let mut notes = Vec::new();
    let mut used = [false; 6];
    for (i, opening) in data.outputs.iter().enumerate() {
        let index = opening.action_index as usize;
        if index >= 6
            || used[index]
            || opening.cmx != identity.native_actions[index]
            || opening.amount != integration.delivery.amounts[i]
        {
            return Err("NU7 delivered output amount/mapping substitution".into());
        }
        used[index] = true;
        let recipient = orchard::Address::from_raw_address_bytes(
            opening
                .recipient
                .as_slice()
                .try_into()
                .map_err(|_| "NU7 recipient length")?,
        )
        .into_option()
        .ok_or("NU7 recipient encoding")?;
        let rho = Rho::from_bytes(&opening.rho)
            .into_option()
            .ok_or("NU7 rho encoding")?;
        let seed = RandomSeed::from_bytes(opening.rseed, &rho)
            .into_option()
            .ok_or("NU7 rseed encoding")?;
        let note = Note::from_parts(
            recipient,
            NoteValue::from_raw(opening.amount),
            rho,
            seed,
            NoteVersion::V3,
        )
        .into_option()
        .ok_or("NU7 V3 byte reconstruction")?;
        if ExtractedNoteCommitment::from(note.commitment()).to_bytes() != opening.cmx {
            return Err("NU7 bytes differ from native commitment".into());
        }
        let mut salt = [0; 32];
        OsRng.fill_bytes(&mut salt);
        rows.push(Row {
            conditions: integration.context.conditions,
            accepted_root: integration.context.accepted_root,
            ticket: if i < 4 {
                integration.context.ticket_ids[i]
            } else {
                [0; 32]
            },
            role: if i < 4 {
                Role::Refund
            } else if i == 4 {
                Role::Seller
            } else {
                Role::Operator
            },
            action_index: opening.action_index,
            gd: recipient.g_d().to_bytes(),
            pkd: recipient.pk_d().inner().to_bytes(),
            amount: opening.amount,
            salt,
        });
        notes.push(note);
    }
    let rows: [_; 6] = rows.try_into().map_err(|_| "six NU7 rows")?;
    let notes: [_; 6] = notes.try_into().map_err(|_| "six NU7 notes")?;
    if !prove {
        return Ok(
            serde_json::json!({"scope":"synthetic NU7 auction preparation and six output-byte reconstruction only",
            "prepared":prepared,"private_fill_delivery_verified":true,"delivery_rule":integration.delivery.rule(),
            "maximum_bids_read":false,"price":integration.delivery.price,"sold":integration.delivery.sold,
            "six_native_output_openings_checked":true,"application_binding_proofs_generated":false,
            "four_receipt_memos_in_same_six_output_pczt":receipts,
            "threshold_signatures_created":false,"chain_funding":false,"broadcast":false,"seconds":start.elapsed().as_secs_f64()}),
        );
    }
    let application_start = Instant::now();
    let approval = if let Some((prover, workers)) = warm {
        integration.prove_and_check_warm(
            &rows,
            &notes,
            identity.native_actions,
            identity.effects_digest,
            prover,
            workers,
        )?
    } else {
        integration.prove_and_check(
            &rows,
            &notes,
            identity.native_actions,
            identity.effects_digest,
        )?
    };
    approval.check(&identity.native_actions, &identity.effects_digest)?;
    let mut changed = identity.effects_digest;
    changed[0] ^= 1;
    if approval.check(&identity.native_actions, &changed).is_ok() {
        return Err("NU7 changed effects accepted".into());
    }
    let application_seconds = application_start.elapsed().as_secs_f64();
    let native = checked_sdk("finalize-auction", directory)?;
    if native["txid"] != identity.txid
        || native["recipients_decrypted"] != 6
        || native["threshold_spends"] != 4
    {
        return Err("NU7 final SDK result identity mismatch".into());
    }
    Ok(
        serde_json::json!({"scope":SCOPE,"passed":true,"prepared":prepared,"native":native,
        "private_fill_delivery_verified":true,"delivery_rule":integration.delivery.rule(),
        "private_winner_flags_verified":integration.delivery.winners.is_some(),"maximum_bids_read":false,
        "price":integration.delivery.price,"sold":integration.delivery.sold,"layout":integration.layout.id(),
        "application_seconds":application_seconds,"seconds":start.elapsed().as_secs_f64(),
        "warm_proving_key_reused":warm.is_some(),"row_workers":warm.map_or(1, |(_, workers)| workers),
        "acceptance_or_recipient_receipts_authenticated":false,"chain_funding":false,"broadcast":false}),
    )
}

pub fn verify_application(path: &Path) -> Result<serde_json::Value> {
    verify_application_inner(path, None)
}
pub fn verify_funded_application(path: &Path, authority: &Path) -> Result<serde_json::Value> {
    verify_application_inner(path, Some(authority))
}
fn verify_application_inner(path: &Path, authority: Option<&Path>) -> Result<serde_json::Value> {
    let start = Instant::now();
    let directory = local(path)?;
    // Expected effects are passed by the trusted SDK caller from its in-memory
    // native transaction, never inferred from an operator approval file alone.
    let mut input = Vec::new();
    std::io::stdin()
        .take(16385)
        .read_to_end(&mut input)
        .map_err(|_| "expected native effects input")?;
    if input.len() > 16384 {
        return Err("expected effects exceed bound".into());
    }
    let actual: Identity =
        serde_json::from_slice(&input).map_err(|_| "strict expected NU7 effects schema")?;
    actual.validate_scope(if authority.is_some() {
        FUNDED_PREPARATION
    } else {
        SCOPE
    })?;
    let saved: Identity = json(&directory, "nu7-native-identity.json", 16384)?;
    if saved != actual {
        return Err("NU7 actual native effects differ from proved identity".into());
    }
    let layout = read_layout(&directory).map_err(|_| "NU7 fixed native layout metadata")?;
    if layout != RowLayout::Shared18 {
        return Err("NU7 auction requires the compiled shared18 layout".into());
    }
    let statement: NativeBatchStatement = json(&directory, "native-public.json", 32768)?;
    if statement.native_actions != actual.native_actions {
        return Err("NU7 statement differs from actual native actions".into());
    }
    link_four_bid_statements(
        &statement.row_hashes,
        &statement.bindings(),
        &actual.native_actions,
    )
    .map_err(|_| "NU7 actual native output/action substitution")?;
    let result: serde_json::Value = json(&directory, "mpc-result.json", 4096)?;
    if result["hash_rule"] != HASH_RULE {
        return Err("NU7 MPC hash rule".into());
    }
    let mpc = local(Path::new(
        result["run"].as_str().ok_or("NU7 MPC public path")?,
    ))?;
    let encoded = read_private(&mpc.join("public-input0"), 232 * 32)?;
    if encoded.len() != 232 * 32 {
        return Err("NU7 MPC public statement size".into());
    }
    if let Some(authority) = authority {
        use darkcall_wallet_bridge::final_book::{Certificate, Manifest};
        let manifest: Manifest = serde_json::from_slice(&read_private(authority, 16384)?)
            .map_err(|_| "strict independently trusted funded authority")?;
        let certificate: Certificate = json(&directory, "final-book-certificate.json", 131072)?;
        certificate.verify_statement(&encoded, &manifest)?;
    }
    let mut hashes = [[0u32; 8]; 6];
    for (i, field) in encoded[184 * 32..].chunks_exact(32).enumerate() {
        if field[..28] != [0; 28] {
            return Err("NU7 row word exceeds u32".into());
        }
        hashes[i / 8][i % 8] = u32::from_be_bytes(field[28..].try_into().unwrap());
    }
    link_four_bid_statements(&hashes, &statement.bindings(), &actual.native_actions)
        .map_err(|_| "NU7 MPC/actual transaction mismatch")?;
    let proofs = (0..6)
        .map(|i| read_private(&directory.join(format!("native-binding-proof-{i}")), 65536))
        .collect::<Result<Vec<_>>>()?;
    verify_set_for_layout(layout, &statement, &proofs)
        .map_err(|_| "NU7 fixed native row proof rejection")?;
    let label = format!(
        "nu7-public-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "verification clock")?
            .as_nanos()
    );
    let snapshot = root().join(".local").join(&label);
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&snapshot)
        .map_err(|_| "NU7 public snapshot")?;
    write_private(&snapshot.join("public-input0"), &encoded)?;
    for name in [
        "vk",
        "proof.0",
        "tampered-price",
        "tampered-plan",
        "tampered-row",
    ] {
        write_private(
            &snapshot.join(name),
            &read_private(&mpc.join(name), 1_048_576)?,
        )?;
    }
    let log = fs::File::create(directory.join(format!("{label}.log")))
        .map_err(|_| "NU7 public verifier log")?;
    let status = Command::new("python3")
        .arg(root().join("scripts/verify_external.py"))
        .arg(&snapshot)
        .args([
            "--circuit",
            "settlement_poseidon4",
            "--output-label",
            &label,
        ])
        .stdout(Stdio::from(log.try_clone().map_err(|_| "log clone")?))
        .stderr(Stdio::from(log))
        .status()
        .map_err(|_| "NU7 C++ verifier launch")?;
    if !status.success() {
        return Err("NU7 fixed C++ auction proof rejected".into());
    }
    Ok(
        serde_json::json!({"scope":"fixed public NU7 auction application verification; native consensus checked by caller",
        "native_row_proofs_verified":true,"normal_cpp_auction_proof_verified":true,"actual_native_effects_and_outputs_match":true,
        "layout":layout.id(),"accepted_book_and_recipients_authenticated":authority.is_some(),"private_openings_keys_pczt_read":false,"chain_funding_or_inclusion_verified":false,
        "seconds":start.elapsed().as_secs_f64()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expected_sdk_effects_require_exact_pinned_identity() {
        let mut value = Identity {
            format: 1,
            scope: SCOPE.into(),
            sdk: SDK.into(),
            branch: "77190ad9".into(),
            txid: "01".repeat(32),
            effects_digest: [1; 32],
            native_actions: [[0; 32]; 6],
        };
        assert!(value.validate().is_ok());
        value.scope = FUNDED_PREPARATION.into();
        assert!(value.validate().is_err());
        assert!(value.validate_scope(FUNDED_PREPARATION).is_ok());
        value.scope = SCOPE.into();
        value.effects_digest[0] ^= 1;
        assert!(value.validate().is_err());
        value.effects_digest[0] ^= 1;
        value.branch = "37a5165b".into();
        assert!(value.validate().is_err());
        let mut encoded = serde_json::to_value(&value).unwrap();
        encoded["approval"] = true.into();
        assert!(serde_json::from_value::<Identity>(encoded).is_err());
    }
}
