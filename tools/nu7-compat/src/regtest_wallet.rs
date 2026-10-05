//! Actual isolated NU7 node captures -> private native notes and Merkle paths.
//! The local node/genesis/birthday are trusted, not SPV-verified. No broadcast
//! or signing, and no synthetic note/witness substitution is supported here.
use crate::{Result, auction, interop_export::write};
use incrementalmerkletree::{frontier::CommitmentTree, witness::IncrementalWitness};
use orchard::{
    Note,
    keys::{FullViewingKey, Scope, SpendingKey},
    note::{ExtractedNoteCommitment, NoteVersion},
    tree::MerkleHashOrchard,
};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Cursor, path::Path};
use zcash_primitives::{merkle_tree::read_commitment_tree, transaction::Transaction};
use zcash_protocol::consensus::BranchId;

const GENESIS: &str = "029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327";
type Tree = CommitmentTree<MerkleHashOrchard, 32>;
type Witness = IncrementalWitness<MerkleHashOrchard, 32>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    height: u32,
    hash: String,
    ironwood_tree: String,
}
impl Snapshot {
    fn tree(&self) -> Result<Tree> {
        if self.ironwood_tree.is_empty() {
            return Ok(Tree::empty());
        }
        if self.ironwood_tree.len() > 16384 {
            return Err("NU7 native frontier size bound".into());
        }
        let bytes = hex::decode(&self.ironwood_tree).map_err(|_| "NU7 frontier encoding")?;
        let mut cursor = Cursor::new(&bytes);
        let tree: Tree = read_commitment_tree(&mut cursor).map_err(|_| "NU7 frontier parse")?;
        if cursor.position() != bytes.len() as u64
            || (tree.left().is_none()
                && (tree.right().is_some() || tree.parents().iter().any(Option::is_some)))
        {
            return Err("NU7 frontier trailing data or orphan nodes".into());
        }
        Ok(tree)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    height: u32,
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
    nu7_height: u32,
    initial: Snapshot,
    final_tree: Snapshot,
    blocks: Vec<Block>,
}
struct Owned {
    note: Note,
    txid: [u8; 32],
    action: usize,
    height: u32,
    block: [u8; 32],
    coinbase: bool,
    witness: Witness,
    nullifier: [u8; 32],
    spent: bool,
}
fn hash(text: &str) -> Result<[u8; 32]> {
    let mut bytes: [u8; 32] = hex::decode(text)
        .map_err(|_| "NU7 block hash encoding")?
        .try_into()
        .map_err(|_| "NU7 block hash length")?;
    bytes.reverse();
    Ok(bytes)
}
pub(crate) fn viewing_key(path: &Path) -> Result<FullViewingKey> {
    let directory = auction::directory(path)?;
    let sk = directory.join("private-spending-key.bin");
    let fvk = directory.join("private-full-viewing-key.bin");
    match (sk.exists(), fvk.exists()) {
        (true, false) => {
            if directory.join("dkg-ready").exists()
                || directory.join("private-frost-public-package.bin").exists()
            {
                return Err("NU7 DKG escrow must not contain a master key".into());
            }
            let bytes = auction::read(&sk, 32)?
                .try_into()
                .map_err(|_| "NU7 wallet key length")?;
            Ok(FullViewingKey::from(
                &SpendingKey::from_bytes(bytes)
                    .into_option()
                    .ok_or("NU7 wallet key encoding")?,
            ))
        }
        (false, true) => {
            if auction::read(&directory.join("dkg-ready"), 64)? != b"DarkCall/local-DKG/v1/ready\n"
            {
                return Err("NU7 DKG setup is not durably complete".into());
            }
            let bytes = auction::read(&fvk, 96)?
                .try_into()
                .map_err(|_| "NU7 FVK length")?;
            FullViewingKey::from_bytes(&bytes).ok_or("NU7 FVK encoding".into())
        }
        _ => Err("NU7 wallet requires exactly one key format".into()),
    }
}

pub fn scan(wallet: &Path, capture_path: &Path) -> Result<serde_json::Value> {
    let directory = auction::directory(wallet)?;
    auction::directory(capture_path.parent().ok_or("NU7 capture parent")?)?;
    let capture: Capture = serde_json::from_slice(&auction::read(capture_path, 100_000_000)?)
        .map_err(|_| "NU7 capture schema")?;
    if capture.genesis != GENESIS
        || capture.branch != "77190ad9"
        || capture.nu7_height != 2
        || capture.initial.height < 2
        || capture.blocks.len() > 500
    {
        return Err("capture differs from pinned isolated NU7 regtest".into());
    }
    // Pinned network scope is checked before key access. DKG wallets use their
    // FVK only; no custody packages are read by this scanner.
    let mut tree = capture.initial.tree()?;
    let expected = capture.final_tree.tree()?;
    let mut previous = hash(&capture.initial.hash)?;
    let mut height = capture.initial.height;
    let fvk = viewing_key(&directory)?;
    let ivks = [Scope::External, Scope::Internal].map(|scope| fvk.to_ivk(scope));
    let mut owned: BTreeMap<[u8; 32], Owned> = BTreeMap::new();
    for block in capture.blocks {
        if block.height != height.checked_add(1).ok_or("NU7 height overflow")?
            || hash(&block.prev_hash)? != previous
            || block.transactions.is_empty()
            || block.transactions.len() > 20000
        {
            return Err("NU7 captured block continuity/order/shape mismatch".into());
        }
        let block_hash = hash(&block.hash)?;
        let _trusted_node_time = block.time;
        let mut action_count = 0usize;
        for (index, text) in block.transactions.into_iter().enumerate() {
            if text.len() > 4_000_000 {
                return Err("NU7 raw transaction size bound".into());
            }
            let bytes = hex::decode(text).map_err(|_| "NU7 transaction encoding")?;
            let mut cursor = Cursor::new(&bytes);
            let tx = Transaction::read(&mut cursor, BranchId::Nu7)
                .map_err(|_| "NU7 transaction parse")?;
            if cursor.position() != bytes.len() as u64 || tx.consensus_branch_id() != BranchId::Nu7
            {
                return Err("NU7 raw transaction branch/trailing data".into());
            }
            let Some(bundle) = tx.ironwood_bundle() else {
                continue;
            };
            action_count = action_count
                .checked_add(bundle.actions().len())
                .filter(|n| *n <= 20000)
                .ok_or("NU7 block native action bound")?;
            for (action_index, action) in bundle.actions().iter().enumerate() {
                let nf = action.nullifier().to_bytes();
                for note in owned.values_mut().filter(|n| n.nullifier == nf) {
                    if note.spent {
                        return Err("NU7 repeated owned nullifier".into());
                    }
                    note.spent = true;
                }
                let leaf = MerkleHashOrchard::from_cmx(action.cmx());
                for note in owned.values_mut().filter(|n| !n.spent) {
                    note.witness
                        .append(leaf)
                        .map_err(|_| "NU7 witness tree full")?;
                }
                tree.append(leaf).map_err(|_| "NU7 commitment tree full")?;
                for key in &ivks {
                    if let Some((note, _, _)) = bundle.decrypt_output_with_key(action_index, key) {
                        if note.version() != NoteVersion::V3
                            || note.value().inner() > 2_100_000_000_000_000
                        {
                            return Err("NU7 received note version/money range".into());
                        }
                        if owned.contains_key(&action.cmx().to_bytes()) {
                            return Err("NU7 repeated owned commitment".into());
                        }
                        owned.insert(
                            action.cmx().to_bytes(),
                            Owned {
                                nullifier: note.nullifier(&fvk).to_bytes(),
                                note,
                                txid: *tx.txid().as_ref(),
                                action: action_index,
                                height: block.height,
                                block: block_hash,
                                coinbase: index == 0,
                                witness: Witness::from_tree(tree.clone())
                                    .ok_or("NU7 empty witness")?,
                                spent: false,
                            },
                        );
                        break;
                    }
                }
            }
        }
        if tree.size() as u64 != u64::from(block.ironwood_size) {
            return Err("NU7 scanned native tree size differs from node".into());
        }
        height = block.height;
        previous = block_hash;
    }
    if height != capture.final_tree.height
        || previous != hash(&capture.final_tree.hash)?
        || tree.size() != expected.size()
        || tree.root() != expected.root()
    {
        return Err("NU7 scanned frontier differs from final node snapshot".into());
    }
    let mut openings = Vec::new();
    let mut balance = 0u64;
    for (cmx, note) in owned.iter().filter(|(_, n)| !n.spent) {
        let path = note.witness.path().ok_or("NU7 missing native path")?;
        let position =
            u32::try_from(u64::from(path.position())).map_err(|_| "NU7 position range")?;
        let auth: [MerkleHashOrchard; 32] =
            path.path_elems().try_into().map_err(|_| "NU7 path depth")?;
        let leaf =
            MerkleHashOrchard::from_cmx(&ExtractedNoteCommitment::from(note.note.commitment()));
        if path.root(leaf) != tree.root() {
            return Err("NU7 native note path differs from canonical anchor".into());
        }
        balance = balance
            .checked_add(note.note.value().inner())
            .ok_or("NU7 balance overflow")?;
        openings.push(serde_json::json!({"cmx":cmx,"txid":note.txid,"action_index":note.action,
            "mined_height":note.height,"mined_block":note.block,"coinbase":note.coinbase,
            "recipient":note.note.recipient().to_raw_address_bytes().to_vec(),"amount":note.note.value().inner(),
            "rho":note.note.rho().to_bytes(),"rseed":note.note.rseed().as_bytes(),
            "position":position,"auth_path":auth.map(|h| h.to_bytes())}));
    }
    let result = serde_json::json!({"scope":"isolated NU7 regtest wallet openings","genesis":GENESIS,
        "branch":"77190ad9","nu7_height":2,"tip_height":height,"tip_hash":capture.final_tree.hash,
        "anchor":tree.root().to_bytes(),"notes":openings});
    let output = directory.join(format!("private-nu7-notes-{height}.json"));
    if output.exists() {
        let persisted: serde_json::Value =
            serde_json::from_slice(&auction::read(&output, 1_000_000)?)
                .map_err(|_| "NU7 persisted note schema")?;
        if persisted != result {
            return Err("NU7 persisted notes differ from fresh canonical scan".into());
        }
    } else {
        write(
            &output,
            &serde_json::to_vec(&result).map_err(|_| "NU7 note encoding")?,
        )?;
    }
    Ok(
        serde_json::json!({"scope":"actual isolated NU7 notes from trusted local Zebra; not public testnet",
        "height":height,"decrypted_unspent_native_notes":openings.len(),"balance_zatoshi":balance,
        "spent_native_notes":owned.values().filter(|n| n.spent).count(),"all_native_paths_match_node_tree":true,
        "funding_observations_fabricated":false,"keys_openings_or_memos_logged":false,
        "native_consensus_source":"pinned local full node, not scanner/SPV proof"}),
    )
}
