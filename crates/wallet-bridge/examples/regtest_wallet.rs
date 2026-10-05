//! Receive actual native notes from the isolated pinned Zebra regtest. Keys and
//! note/witness openings stay in a new private ignored directory, never stdout.
use darkcall_wallet_bridge::scanner::Scanner;
use orchard::{
    keys::{FullViewingKey, Scope, SpendingKey},
    note::NoteVersion,
};
use rand::{RngCore, rngs::OsRng};
use serde::Deserialize;
use std::{fs, io::Write, path::Path};
use zcash_address::unified::{Address, Encoding, Receiver};
use zcash_client_backend::proto::{
    compact_formats::{ChainMetadata, CompactBlock, CompactOrchardAction, CompactTx},
    service::TreeState,
};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BranchId, NetworkType};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const GENESIS: &str = "029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327";
fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut f = options.open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}
fn check_dir(path: &Path) -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    if !path
        .canonicalize()?
        .starts_with(root.join(".local").canonicalize()?)
        || path.is_symlink()
        || !fs::metadata(path)?.is_dir()
    {
        return Err("wallet must be a private ignored local directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path)?.permissions().mode() & 0o077 != 0 {
            return Err("wallet directory must be 0700".into());
        }
    }
    Ok(())
}
fn read_key(path: &Path) -> Result<SpendingKey> {
    check_dir(path)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let f = options.open(path.join("private-spending-key.bin"))?;
    let meta = f.metadata()?;
    if !meta.is_file() || meta.len() != 32 {
        return Err("invalid private wallet key file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.permissions().mode() & 0o077 != 0 || meta.nlink() != 1 {
            return Err("wallet key must be private with one link".into());
        }
    }
    use std::io::Read;
    let mut bytes = [0; 32];
    (&f).read_exact(&mut bytes)?;
    SpendingKey::from_bytes(bytes)
        .into_option()
        .ok_or_else(|| "invalid native spending key".into())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tree {
    height: u64,
    hash: String,
    ironwood_tree: String,
}
impl Tree {
    fn sdk(&self) -> TreeState {
        TreeState {
            network: "test".into(),
            height: self.height,
            hash: self.hash.clone(),
            ironwood_tree: self.ironwood_tree.clone(),
            ..Default::default()
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    height: u64,
    hash: String,
    prev_hash: String,
    time: u32,
    ironwood_size: u32,
    transactions: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    genesis: String,
    branch: String,
    nu6_3_height: u32,
    initial: Tree,
    final_tree: Tree,
    blocks: Vec<Block>,
}
fn hash_le(text: &str) -> Result<Vec<u8>> {
    let mut b = hex::decode(text)?;
    if b.len() != 32 {
        return Err("hash length".into());
    }
    b.reverse();
    Ok(b)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: regtest_wallet init DIRECTORY | scan DIRECTORY NODE_CAPTURE".into());
    }
    let directory = Path::new(&args[1]);
    if args[0] == "init" && args.len() == 2 {
        if !directory.exists() {
            // The caller creates this inside the already ignored private .local tree.
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(".local")
                .canonicalize()?;
            if !directory
                .parent()
                .ok_or("wallet parent")?
                .canonicalize()?
                .starts_with(root)
            {
                return Err("wallet parent outside .local".into());
            }
            fs::create_dir(directory)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
            }
            let key = loop {
                let mut bytes = [0; 32];
                OsRng.fill_bytes(&mut bytes);
                if let Some(key) = SpendingKey::from_bytes(bytes).into_option() {
                    break key;
                }
            };
            private_write(&directory.join("private-spending-key.bin"), key.to_bytes())?;
            fs::File::open(directory)?.sync_all()?;
            fs::File::open(directory.parent().ok_or("wallet parent")?)?.sync_all()?;
        }
        let fvk = FullViewingKey::from(&read_key(directory)?);
        let receiver = fvk.address_at(0u32, Scope::External).to_raw_address_bytes();
        let address = Address::try_from_items(vec![Receiver::Orchard(receiver)])?
            .encode(&NetworkType::Regtest);
        println!(
            "{}",
            serde_json::json!({"scope":"isolated regtest only", "address":address,"receiver":receiver.to_vec(),
                "internal_receiver":fvk.address_at(1u32,Scope::Internal).to_raw_address_bytes().to_vec(),"secret_key_logged":false})
        );
        return Ok(());
    }
    if args[0] != "scan" || args.len() != 3 {
        return Err("unknown regtest wallet command".into());
    }
    let fvk = FullViewingKey::from(&read_key(directory)?);
    let capture_path = Path::new(&args[2]);
    if fs::metadata(capture_path)?.len() > 100_000_000 {
        return Err("node capture too large".into());
    }
    let capture: Capture = serde_json::from_slice(&fs::read(capture_path)?)?;
    if capture.genesis != GENESIS
        || capture.branch != "37a5165b"
        || capture.nu6_3_height != 2
        || capture.initial.height < 2
        || capture.blocks.len() > 500
    {
        return Err("capture differs from pinned regtest scope".into());
    }
    let mut scanner = Scanner::from_tree_state(&capture.initial.sdk())?;
    for block in capture.blocks {
        let mut txs = Vec::new();
        for (index, text) in block.transactions.into_iter().enumerate() {
            let bytes = hex::decode(text)?;
            let mut cursor = std::io::Cursor::new(&bytes);
            let tx = Transaction::read(&mut cursor, BranchId::Nu6_3)?;
            if cursor.position() != bytes.len() as u64 {
                return Err("transaction trailing bytes".into());
            }
            let actions = tx
                .ironwood_bundle()
                .map(|bundle| {
                    bundle
                        .actions()
                        .iter()
                        .map(|a| CompactOrchardAction {
                            nullifier: a.nullifier().to_bytes().to_vec(),
                            cmx: a.cmx().to_bytes().to_vec(),
                            ephemeral_key: a.encrypted_note().epk_bytes.to_vec(),
                            ciphertext: a.encrypted_note().enc_ciphertext[..52].to_vec(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            txs.push(CompactTx {
                index: index as u64,
                txid: tx.txid().as_ref().to_vec(),
                ironwood_actions: actions,
                ..Default::default()
            });
        }
        scanner.apply(
            &CompactBlock {
                height: block.height,
                hash: hash_le(&block.hash)?,
                prev_hash: hash_le(&block.prev_hash)?,
                time: block.time,
                vtx: txs,
                chain_metadata: Some(ChainMetadata {
                    ironwood_commitment_tree_size: block.ironwood_size,
                    ..Default::default()
                }),
                ..Default::default()
            },
            &fvk,
        )?;
    }
    scanner.verify_tree_state(&capture.final_tree.sdk())?;
    let mut openings = Vec::new();
    let mut balance = 0u64;
    for (cmx, owned) in scanner.notes() {
        if owned.spent_in.is_some() {
            continue;
        }
        let path = owned.path()?;
        if path.root(owned.note.commitment().into()) != scanner.anchor()
            || owned.note.version() != NoteVersion::V3
        {
            return Err("native witness/version differs from actual scanned tree".into());
        }
        balance = balance
            .checked_add(owned.note.value().inner())
            .ok_or("balance overflow")?;
        openings.push(serde_json::json!({"cmx":cmx,"txid":owned.txid,"action_index":owned.action_index,
            "mined_height":owned.mined_height,"mined_block":owned.mined_block,"coinbase":owned.coinbase,
            "recipient":owned.note.recipient().to_raw_address_bytes().to_vec(),"amount":owned.note.value().inner(),
            "rho":owned.note.rho().to_bytes(),"rseed":owned.note.rseed().as_bytes(),
            "position":path.position(),"auth_path":path.auth_path().map(|h|h.to_bytes())}));
    }
    let private = serde_json::json!({"scope":"isolated regtest wallet openings", "genesis":GENESIS,"nu6_3_height":2,
        "tip_height":scanner.height(),"tip_hash":capture.final_tree.hash,"anchor":scanner.anchor().to_bytes(),"notes":openings});
    private_write(
        &directory.join(format!("private-notes-{}.json", scanner.height())),
        &serde_json::to_vec_pretty(&private)?,
    )?;
    println!(
        "{}",
        serde_json::json!({"scope":"actual isolated regtest notes from trusted local Zebra; not standard testnet",
        "height":scanner.height(),"decrypted_unspent_native_notes":private["notes"].as_array().unwrap().len(),
        "balance_zatoshi":balance,"spent_native_notes":scanner.notes().values().filter(|n|n.spent_in.is_some()).count(),"all_native_paths_match_node_tree":true,"private_keys_or_openings_logged":false,
        "funding_observations_fabricated":false,"chain_inclusion_source":"pinned local full node RPC, not an SPV proof"})
    );
    Ok(())
}
