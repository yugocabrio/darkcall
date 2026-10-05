//! Auction proof adapter for the separately pinned NU7 SDK. All cross-version
//! values are canonical bytes; this working SDK never parses a NU7 transaction.
use super::*;
use crate::regtest_spend::{read_private, write_private};
use darkcall_output_binding::poseidon_rows::{
    HASH_RULE, RowLayout, read_layout, verify_set_for_layout,
};
use darkcall_settlement_types::{NativeBatchStatement, link_four_bid_statements};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const SDK: &str = "9c5705f56517c040c50f96d29f1ff15a9383f3e0";
const SCOPE: &str = "synthetic NU7 six-output auction; no chain funding";
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
        // Shielded-only v6 sighash equals the raw txid. NU7 adapter rejects
        // transparent and other-pool bundles before producing this identity.
        let mut reversed = self.effects_digest;
        reversed.reverse();
        if self.format != 1
            || self.scope != SCOPE
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

pub fn run(book: &Path, name: &str, prove: bool) -> Result<serde_json::Value> {
    let start = Instant::now();
    let integration = Integration::new_with_layout(book, name, RowLayout::Shared18)?;
    let directory = &integration.directory;
    write_private(&directory.join("nu7-auction-request.json"), &serde_json::to_vec(&serde_json::json!({
        "format":1,"scope":SCOPE,"sdk":SDK,"branch":"77190ad9","amounts":integration.delivery.amounts
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
            "threshold_signatures_created":false,"chain_funding":false,"broadcast":false,"seconds":start.elapsed().as_secs_f64()}),
        );
    }
    let application_start = Instant::now();
    let approval = integration.prove_and_check(
        &rows,
        &notes,
        identity.native_actions,
        identity.effects_digest,
    )?;
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
        "acceptance_or_recipient_receipts_authenticated":false,"chain_funding":false,"broadcast":false}),
    )
}

pub fn verify_application(path: &Path) -> Result<serde_json::Value> {
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
    actual.validate()?;
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
        "layout":layout.id(),"private_openings_keys_pczt_read":false,"chain_funding_or_inclusion_verified":false,
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
