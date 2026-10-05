//! Ignored private note-byte export and public NU7 transaction/output checks.
//! No PCZT, spending key or funding; these synthetic transactions must not broadcast.
use crate::Result;
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

pub const SDK: &str = "9c5705f56517c040c50f96d29f1ff15a9383f3e0";

fn local() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".local")
        .canonicalize()
        .map_err(|_| "missing ignored local directory".into())
}

pub fn create(name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || name.len() > 50
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("expected a fresh simple ignored export name".into());
    }
    let directory = local()?.join(name);
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|_| "export directory exists or cannot be created")?;
    Ok(directory)
}

pub fn write(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "cannot create new private export file")?;
    file.write_all(data)
        .and_then(|()| file.sync_all())
        .map_err(|_| "export write/sync failed")?;
    fs::File::open(path.parent().unwrap())
        .and_then(|dir| dir.sync_all())
        .map_err(|_| "export parent sync failed".into())
}

fn read_public(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "public export unavailable")?;
    if !metadata.file_type().is_file()
        || metadata.len() > limit
        || !path
            .canonicalize()
            .map_err(|_| "export path unavailable")?
            .starts_with(local()?)
    {
        return Err("expected bounded regular ignored public export file".into());
    }
    let file = fs::File::open(path).map_err(|_| "public export read failed")?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "public export read failed")?;
    if bytes.len() as u64 > limit {
        return Err("public export exceeds bound".into());
    }
    Ok(bytes)
}

pub fn verify(directory: &Path, row: Option<&Path>) -> Result<serde_json::Value> {
    let bytes = read_public(&directory.join("synthetic-nu7-transaction.bin"), 100_000)?;
    let tx = crate::native::verify_bytes(
        &bytes,
        zcash_protocol::consensus::BranchId::Nu7,
        &mut rand::rng(),
    )?;
    let statement: serde_json::Value = serde_json::from_slice(&read_public(
        &directory.join("public-output-map.json"),
        16384,
    )?)
    .map_err(|_| "public output mapping format")?;
    let keys = [
        "format",
        "scope",
        "sdk",
        "branch",
        "txid",
        "native_actions",
        "outputs",
    ];
    let object = statement
        .as_object()
        .ok_or("public output mapping object")?;
    if object.len() != keys.len()
        || keys.iter().any(|k| !object.contains_key(*k))
        || statement["format"] != 1
        || statement["scope"] != "synthetic NU7 output-byte interoperability"
        || statement["sdk"] != SDK
        || statement["branch"] != "77190ad9"
        || statement["txid"] != tx.txid().to_string()
    {
        return Err("public native mapping identity mismatch".into());
    }
    let bundle = tx.ironwood_bundle().ok_or("native bundle missing")?;
    let actions: Vec<[u8; 32]> = serde_json::from_value(statement["native_actions"].clone())
        .map_err(|_| "native commitments format")?;
    if actions
        != bundle
            .actions()
            .iter()
            .map(|a| a.cmx().to_bytes())
            .collect::<Vec<_>>()
    {
        return Err("output map differs from actual native transaction".into());
    }
    let outputs = statement["outputs"]
        .as_array()
        .ok_or("mapped outputs missing")?;
    if outputs.len() != 2 || actions.len() != 2 {
        return Err("synthetic mapping expects two outputs/actions".into());
    }
    let mut used = std::collections::BTreeSet::new();
    for output in outputs {
        if output.as_object().is_none_or(|v| {
            v.len() != 2 || !v.contains_key("action_index") || !v.contains_key("cmx")
        }) {
            return Err("expected public output mapping fields only".into());
        }
        let index = output["action_index"]
            .as_u64()
            .ok_or("action index missing")?;
        let cmx: [u8; 32] =
            serde_json::from_value(output["cmx"].clone()).map_err(|_| "cmx encoding")?;
        if index >= actions.len() as u64 || !used.insert(index) || cmx != actions[index as usize] {
            return Err("mapped commitment/index differs from serialized native action".into());
        }
    }
    if let Some(path) = row {
        // Only public row inputs; no private-native-outputs file is opened.
        let mut value: serde_json::Value = serde_json::from_slice(&read_public(path, 16384)?)
            .map_err(|_| "public binding statement")?;
        if value.get("layout").is_some() {
            let wrapper = value.as_object().ok_or("shared public wrapper")?;
            if wrapper.len() != 2 || !wrapper.contains_key("statement") ||
                value["layout"] != "experimental-shared18-native-row-v1" {
                return Err("unsupported shared public circuit identity".into());
            }
            value = value["statement"].clone();
        }
        let minimal = ["action_index", "cmx", "branch", "txid"];
        let full = [
            "format",
            "scope",
            "sdk",
            "branch",
            "txid",
            "row_hash_words",
            "cmx",
            "action_index",
        ];
        let object = value.as_object().ok_or("public row mapping object")?;
        let keys: &[&str] = if object.len() == minimal.len() {
            &minimal
        } else {
            &full
        };
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err("expected public row fields only".into());
        }
        if object.len() == full.len() {
            if value["format"] != 1
                || value["sdk"] != SDK
                || value["scope"]
                    != "one new-SDK synthetic native output binds to BN254 row; no auction admission/funding"
            {
                return Err("unsupported public row compatibility identity".into());
            }
            serde_json::from_value::<[u32; 8]>(value["row_hash_words"].clone())
                .map_err(|_| "public row digest word encoding")?;
        }
        let index = value["action_index"]
            .as_u64()
            .ok_or("binding action index")?;
        let cmx: [u8; 32] =
            serde_json::from_value(value["cmx"].clone()).map_err(|_| "binding cmx")?;
        if value["txid"] != tx.txid().to_string()
            || value["branch"] != "77190ad9"
            || index >= actions.len() as u64
            || cmx != actions[index as usize]
        {
            return Err("binding row differs from actual NU7 native output".into());
        }
    }
    Ok(
        serde_json::json!({"scope":"public-only NU7 native proof/signature/output mapping check; row proof verified separately by working circuit",
        "native_proof_and_signatures_verified":true,"actual_serialized_cmx_mapping_verified":true,
        "public_row_compared_to_actual_output":row.is_some(),"row_proof_verified_here":false,"private_openings_read":false,
        "chain_funding_or_inclusion_verified":false,"broadcast":false,"txid":tx.txid().to_string()}),
    )
}
