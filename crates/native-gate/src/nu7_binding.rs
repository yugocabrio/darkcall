//! Existing working binding circuit against new-SDK synthetic V3 output bytes.
//! The new SDK verifies the NU7 tx separately; this old SDK cannot parse NU7.
use super::*;
use crate::regtest_spend::{read_private, write_private};
use darkcall_bn254_emulation::poseidon::{digest_words, pack_row, reference};
use darkcall_output_binding::poseidon_rows::{RowProver, binding_from_note, verify_one};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const SDK: &str = "9c5705f56517c040c50f96d29f1ff15a9383f3e0";
const SCOPE: &str =
    "one new-SDK synthetic native output binds to BN254 row; no auction admission/funding";

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
struct Input {
    format: u32,
    scope: String,
    sdk: String,
    branch: String,
    txid: String,
    outputs: Vec<Opening>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Public {
    format: u32,
    scope: String,
    sdk: String,
    branch: String,
    txid: String,
    row_hash_words: [u32; 8],
    cmx: [u8; 32],
    action_index: u32,
}
impl Public {
    fn inputs(&self) -> Result<Vec<pallas::Base>> {
        if self.format != 1
            || self.scope != SCOPE
            || self.sdk != SDK
            || self.branch != "77190ad9"
            || self.action_index >= 2
            || hex::decode(&self.txid).map_or(true, |bytes| bytes.len() != 32)
        {
            return Err("NU7 output compatibility statement identity".into());
        }
        let cmx = ExtractedNoteCommitment::from_bytes(&self.cmx)
            .into_option()
            .ok_or("noncanonical output cmx")?;
        let mut inputs: Vec<_> = self
            .row_hash_words
            .iter()
            .map(|x| pallas::Base::from(*x as u64))
            .collect();
        inputs.extend([cmx.inner(), pallas::Base::from(self.action_index as u64)]);
        Ok(inputs)
    }
}
fn local(path: &Path) -> Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let path = path
        .canonicalize()
        .map_err(|_| "ignored compatibility directory missing")?;
    if !path.starts_with(root.join(".local")) || !path.is_dir() {
        return Err("expected ignored compatibility directory".into());
    }
    Ok(path)
}

pub fn prove(directory: &Path, name: &str) -> Result<serde_json::Value> {
    let started = Instant::now();
    let directory = local(directory)?;
    if name.is_empty()
        || name.len() > 50
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("expected fresh simple ignored binding name".into());
    }
    let data: Input = serde_json::from_slice(&read_private(
        &directory.join("private-native-outputs.json"),
        16384,
    )?)
    .map_err(|_| "private NU7 output byte schema")?;
    if data.format != 1
        || data.scope != "synthetic NU7 output-byte interoperability"
        || data.sdk != SDK
        || data.branch != "77190ad9"
        || data.outputs.len() != 2
    {
        return Err("unsupported new-SDK output fixture".into());
    }
    let opening = &data.outputs[0];
    let recipient: [u8; 43] = opening
        .recipient
        .as_slice()
        .try_into()
        .map_err(|_| "recipient length")?;
    let recipient = orchard::Address::from_raw_address_bytes(&recipient)
        .into_option()
        .ok_or("recipient encoding")?;
    let rho = Rho::from_bytes(&opening.rho)
        .into_option()
        .ok_or("rho encoding")?;
    let seed = RandomSeed::from_bytes(opening.rseed, &rho)
        .into_option()
        .ok_or("rseed encoding")?;
    let note = Note::from_parts(
        recipient,
        NoteValue::from_raw(opening.amount),
        rho,
        seed,
        NoteVersion::V3,
    )
    .into_option()
    .ok_or("new note bytes rejected by working binding stack")?;
    if ExtractedNoteCommitment::from(note.commitment()).to_bytes() != opening.cmx {
        return Err("new output bytes do not match native commitment".into());
    }
    let mut salt = [0; 32];
    OsRng.fill_bytes(&mut salt);
    let row = Row {
        conditions: [1; 32],
        accepted_root: [2; 32],
        ticket: [3; 32],
        role: Role::Refund,
        action_index: opening.action_index,
        gd: recipient.g_d().to_bytes(),
        pkd: recipient.pk_d().inner().to_bytes(),
        amount: opening.amount,
        salt,
    };
    let public = Public {
        format: 1,
        scope: SCOPE.into(),
        sdk: SDK.into(),
        branch: data.branch,
        txid: data.txid,
        row_hash_words: digest_words(&reference(&pack_row(&row.encode()))),
        cmx: opening.cmx,
        action_index: opening.action_index,
    };
    let inputs = public.inputs()?;
    let parent = directory.parent().ok_or("ignored output parent")?;
    let output = parent.join(name);
    fs::create_dir(&output).map_err(|_| "compatibility output exists or cannot be created")?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&output, fs::Permissions::from_mode(0o700))
        .map_err(|_| "private output permissions")?;
    let mut changed = row.clone();
    changed.amount = changed.amount.checked_add(1).ok_or("amount overflow")?;
    if binding_from_note(&changed, &note).is_ok() {
        return Err("changed row amount accepted".into());
    }
    changed = row.clone();
    changed.gd[0] ^= 1;
    if binding_from_note(&changed, &note).is_ok() {
        return Err("changed row recipient accepted".into());
    }
    eprintln!("Existing native binding key generation for new-SDK output started");
    let key_start = Instant::now();
    let prover = RowProver::new().map_err(|e| error("trusted native row key", e))?;
    let key_seconds = key_start.elapsed().as_secs_f64();
    let proof_start = Instant::now();
    let proof = prover
        .prove(
            binding_from_note(&row, &note).map_err(|e| error("byte-derived native witness", e))?,
            &inputs,
        )
        .map_err(|e| error("NU7 output row proof", e))?;
    let proof_seconds = proof_start.elapsed().as_secs_f64();
    for i in [0, 8, 9] {
        let mut changed = inputs.clone();
        changed[i] += pallas::Base::ONE;
        if prover.verify(&proof, &changed).is_ok() {
            return Err("changed hash/cmx/index accepted".into());
        }
    }
    write_private(&output.join("native-binding-proof"), &proof)?;
    write_private(
        &output.join("nu7-row-public.json"),
        &serde_json::to_vec(&public).map_err(|_| "public row encoding")?,
    )?;
    Ok(
        serde_json::json!({"scope":SCOPE,"new_sdk_native_output_opening":true,"working_binding_circuit_unchanged":true,
        "native_cmx_matches":true,"proof_verified":true,"proof_bytes":proof.len(),"changed_hash_cmx_index_rejected":true,
        "changed_amount_recipient_rejected_before_proving":true,"native_consensus_proof_verified_here":false,
        "auction_plan_connected":false,"chain_funding":false,"broadcast":false,"key_seconds":key_seconds,
        "proof_seconds":proof_seconds,"seconds":started.elapsed().as_secs_f64()}),
    )
}

pub fn verify(directory: &Path) -> Result<serde_json::Value> {
    let start = Instant::now();
    let directory = local(directory)?;
    // Only public inputs and proof; private-native-outputs/row are not read.
    let public: Public = serde_json::from_slice(&read_private(
        &directory.join("nu7-row-public.json"),
        16384,
    )?)
    .map_err(|_| "public compatibility row schema")?;
    let proof = read_private(&directory.join("native-binding-proof"), 65536)?;
    verify_one(&proof, &public.inputs()?)
        .map_err(|e| error("public new-output binding proof", e))?;
    Ok(
        serde_json::json!({"scope":SCOPE,"public_only_native_row_proof_verified":true,"private_openings_read":false,
        "native_consensus_verified_here":false,"transaction_cmx_mapping_verified_here":false,
        "auction_plan_connected":false,"broadcast":false,"seconds":start.elapsed().as_secs_f64()}),
    )
}
