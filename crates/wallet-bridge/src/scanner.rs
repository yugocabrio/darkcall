//! Incremental Ironwood compact-block decryption and native Merkle witnesses.
//! The provider/initial tree are trusted; this does not verify chain consensus.
//! Persist funding events separately and rebuild this scanner on recovery from
//! retained blocks. Do not publish note openings, values or witnesses.
use crate::journal::Event;
use darkcall_auction_core::MAX_MONEY;
use incrementalmerkletree::{frontier::CommitmentTree, witness::IncrementalWitness};
use orchard::{
    Note,
    keys::{FullViewingKey, Scope},
    note_encryption::{CompactAction, IronwoodDomain},
    tree::{MerkleHashOrchard, MerklePath},
};
use std::collections::BTreeMap;
use zcash_client_backend::proto::{compact_formats::CompactBlock, service::TreeState};
use zcash_note_encryption::try_compact_note_decryption;
use zcash_primitives::merkle_tree::read_commitment_tree;

type Result<T> = std::result::Result<T, String>;
type Tree = CommitmentTree<MerkleHashOrchard, 32>;
type Witness = IncrementalWitness<MerkleHashOrchard, 32>;
const MAX_ACTIONS_PER_BLOCK: usize = 20_000;

#[derive(Clone)]
pub struct OwnedNote {
    pub note: Note,
    pub txid: [u8; 32],
    pub action_index: u32,
    pub mined_height: u32,
    pub mined_block: [u8; 32],
    pub scope: Scope,
    pub spent_in: Option<([u8; 32], u32)>,
    nullifier: [u8; 32],
    witness: Witness,
}
impl OwnedNote {
    /// Native spend path, usable only with the canonical anchor checked by the
    /// caller. Note that an in-memory path is not proof of chain finality.
    pub fn path(&self) -> Result<MerklePath> {
        if self.spent_in.is_some() {
            return Err("note was spent in scanned history".into());
        }
        let path = self.witness.path().ok_or("missing native witness")?;
        let position =
            u32::try_from(u64::from(path.position())).map_err(|_| "native position exceeds u32")?;
        let elements = path
            .path_elems()
            .try_into()
            .map_err(|_| "native path depth mismatch")?;
        Ok(MerklePath::from_parts(position, elements))
    }
}

pub struct ScanDelta {
    pub observations: Vec<Event>,
    /// Caller must reconcile these with reserved effects/cancellation/finality,
    /// or pause the ledger. Never silently continue with spent accepted funding.
    pub spent_notes: Vec<([u8; 32], [u8; 32])>,
}
#[derive(Clone)]
pub struct Scanner {
    height: u32,
    hash: [u8; 32],
    tree: Tree,
    owned: BTreeMap<[u8; 32], OwnedNote>,
}
fn parse_tree(state: &TreeState) -> Result<Tree> {
    if state.ironwood_tree.is_empty() {
        return Ok(Tree::empty());
    }
    if state.ironwood_tree.len() > 16_384 {
        return Err("Ironwood frontier too large".into());
    }
    let bytes = hex::decode(&state.ironwood_tree).map_err(|_| "malformed Ironwood frontier hex")?;
    let mut cursor = std::io::Cursor::new(&bytes);
    let tree = read_commitment_tree(&mut cursor).map_err(|_| "malformed Ironwood frontier")?;
    if cursor.position() != bytes.len() as u64 {
        return Err("Ironwood frontier has trailing bytes".into());
    }
    // The SDK's legacy from_parts parser only bounds parent depth. size() can
    // panic for a right leaf without a left leaf; an orphan parent also cannot
    // describe the most recent leaf. Reject both before calling tree methods.
    if tree.left().is_none()
        && (tree.right().is_some() || tree.parents().iter().any(Option::is_some))
    {
        return Err("Ironwood frontier has orphan leaves/parents".into());
    }
    Ok(tree)
}
impl Scanner {
    /// Initial TreeState belongs to the explicitly verified network and the
    /// block immediately BEFORE the scanning birthday. FVK keys stay with caller.
    pub fn from_tree_state(state: &TreeState) -> Result<Self> {
        if state.network != "test" {
            return Err("scanner accepts explicitly selected test chains only".into());
        }
        let height = u32::try_from(state.height).map_err(|_| "scanner height overflow")?;
        let mut hash = hex::decode(&state.hash).map_err(|_| "malformed checkpoint hash")?;
        hash.reverse();
        let hash = hash
            .try_into()
            .map_err(|_| "checkpoint hash must be 32 bytes")?;
        Ok(Self {
            height,
            hash,
            tree: parse_tree(state)?,
            owned: BTreeMap::new(),
        })
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn notes(&self) -> &BTreeMap<[u8; 32], OwnedNote> {
        &self.owned
    }
    pub fn anchor(&self) -> orchard::Anchor {
        self.tree.root().into()
    }
    pub fn verify_tree_state(&self, state: &TreeState) -> Result<()> {
        let mut hash = self.hash;
        hash.reverse();
        if state.network != "test"
            || state.height != u64::from(self.height)
            || state.hash.to_ascii_lowercase() != hex::encode(hash)
        {
            return Err("tree snapshot and scanned block differ".into());
        }
        let tree = parse_tree(state)?;
        if tree.size() != self.tree.size() || tree.root() != self.tree.root() {
            return Err("Ironwood compact commitments disagree with provider frontier".into());
        }
        Ok(())
    }
    pub fn apply(&mut self, block: &CompactBlock, fvk: &FullViewingKey) -> Result<ScanDelta> {
        // Validate on a clone: a malformed late action or tree-size mismatch
        // must not advance the cursor or retain half of the block's notes.
        let mut next = self.clone();
        let delta = next.apply_inner(block, fvk)?;
        *self = next;
        Ok(delta)
    }
    fn apply_inner(&mut self, block: &CompactBlock, fvk: &FullViewingKey) -> Result<ScanDelta> {
        if block.height
            != u64::from(
                self.height
                    .checked_add(1)
                    .ok_or("scanner height overflow")?,
            )
            || block.hash.len() != 32
            || block.prev_hash.as_slice() != self.hash
            || !block.header.is_empty()
            || block.vtx.len() > MAX_ACTIONS_PER_BLOCK
        {
            return Err("compact block continuity/shape mismatch; explicit rescan required".into());
        }
        let height = u32::try_from(block.height).map_err(|_| "scanner height overflow")?;
        let block_hash: [u8; 32] = block
            .hash
            .as_slice()
            .try_into()
            .map_err(|_| "block hash length")?;
        let metadata = block
            .chain_metadata
            .as_ref()
            .ok_or("missing compact tree metadata")?;
        let mut observed = Vec::new();
        let mut spent = Vec::new();
        let keys =
            [Scope::External, Scope::Internal].map(|scope| (scope, fvk.to_ivk(scope).prepare()));
        let mut previous_index = None;
        let mut action_count = 0usize;
        for tx in &block.vtx {
            if previous_index.is_some_and(|previous| tx.index <= previous) {
                return Err("compact transaction order mismatch".into());
            }
            previous_index = Some(tx.index);
            let txid: [u8; 32] = tx
                .txid
                .as_slice()
                .try_into()
                .map_err(|_| "compact txid length")?;
            action_count = action_count
                .checked_add(tx.ironwood_actions.len())
                .filter(|n| *n <= MAX_ACTIONS_PER_BLOCK)
                .ok_or("too many compact actions")?;
            for (index, encoded) in tx.ironwood_actions.iter().enumerate() {
                let action: CompactAction = encoded
                    .try_into()
                    .map_err(|_| "invalid canonical Ironwood action encoding")?;
                let nf = action.nullifier().to_bytes();
                for (id, note) in &mut self.owned {
                    if note.nullifier == nf {
                        if note.spent_in.is_some() {
                            return Err("owned note nullifier repeated in scanned history".into());
                        }
                        note.spent_in = Some((txid, height));
                        spent.push((*id, txid));
                    }
                }
                let leaf = MerkleHashOrchard::from_cmx(&action.cmx());
                for note in self.owned.values_mut().filter(|n| n.spent_in.is_none()) {
                    note.witness
                        .append(leaf)
                        .map_err(|_| "native witness tree full")?;
                }
                self.tree.append(leaf).map_err(|_| "Ironwood tree full")?;
                let domain = IronwoodDomain::for_compact_action(&action);
                for (scope, key) in &keys {
                    if let Some((note, _address)) =
                        try_compact_note_decryption(&domain, key, &action)
                    {
                        if note.value().inner() > MAX_MONEY {
                            return Err("received note exceeds money range".into());
                        }
                        let id = action.cmx().to_bytes();
                        if self.owned.contains_key(&id) {
                            return Err("owned note commitment repeated".into());
                        }
                        observed.push(Event::Observe {
                            note: id,
                            value: note.value().inner(),
                            block: block_hash,
                            height,
                        });
                        let nullifier = note.nullifier(fvk).to_bytes();
                        self.owned.insert(
                            id,
                            OwnedNote {
                                note,
                                txid,
                                action_index: u32::try_from(index)
                                    .map_err(|_| "action index overflow")?,
                                mined_height: height,
                                mined_block: block_hash,
                                scope: *scope,
                                spent_in: None,
                                nullifier,
                                witness: Witness::from_tree(self.tree.clone())
                                    .ok_or("empty native witness")?,
                            },
                        );
                        break;
                    }
                }
            }
        }
        if u64::from(metadata.ironwood_commitment_tree_size) != self.tree.size() as u64 {
            return Err("compact Ironwood tree-size mismatch".into());
        }
        self.height = height;
        self.hash = block_hash;
        Ok(ScanDelta {
            observations: observed,
            spent_notes: spent,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchard::{
        keys::SpendingKey,
        note::{NoteVersion, Nullifier, RandomSeed, Rho},
        note_encryption::IronwoodNoteEncryption,
        value::NoteValue,
    };
    use rand::{RngCore, rngs::OsRng};
    use zcash_client_backend::proto::compact_formats::{
        ChainMetadata, CompactOrchardAction, CompactTx,
    };
    use zcash_note_encryption::Domain;
    use zcash_primitives::merkle_tree::write_commitment_tree;
    fn key(value: u8) -> FullViewingKey {
        FullViewingKey::from(&SpendingKey::from_bytes([value; 32]).unwrap())
    }
    fn action(fvk: &FullViewingKey, nf: Nullifier, scope: Scope) -> CompactOrchardAction {
        let rho = Rho::from_bytes(&nf.to_bytes()).unwrap();
        let seed = loop {
            let mut seed = [0; 32];
            OsRng.fill_bytes(&mut seed);
            if let Some(seed) = RandomSeed::from_bytes(seed, &rho).into_option() {
                break seed;
            }
        };
        let note = Note::from_parts(
            fvk.address_at(0u32, scope),
            NoteValue::from_raw(15100),
            rho,
            seed,
            NoteVersion::V3,
        )
        .unwrap();
        let encryption = IronwoodNoteEncryption::new(None, note, [0; 512]);
        CompactOrchardAction {
            nullifier: nf.to_bytes().to_vec(),
            cmx: action_cmx(note),
            ephemeral_key: IronwoodDomain::epk_bytes(encryption.epk()).0.to_vec(),
            ciphertext: encryption.encrypt_note_plaintext()[..52].to_vec(),
        }
    }
    fn action_cmx(note: Note) -> Vec<u8> {
        orchard::note::ExtractedNoteCommitment::from(note.commitment())
            .to_bytes()
            .to_vec()
    }
    fn initial() -> TreeState {
        TreeState {
            network: "test".into(),
            height: 4_134_000,
            hash: hex::encode([8; 32]),
            ironwood_tree: "".into(),
            ..Default::default()
        }
    }
    fn block(
        actions: Vec<CompactOrchardAction>,
        height: u64,
        prev: u8,
        hash: u8,
        size: u32,
    ) -> CompactBlock {
        CompactBlock {
            height,
            hash: vec![hash; 32],
            prev_hash: vec![prev; 32],
            vtx: vec![CompactTx {
                index: 1,
                txid: vec![7; 32],
                ironwood_actions: actions,
                ..Default::default()
            }],
            chain_metadata: Some(ChainMetadata {
                ironwood_commitment_tree_size: size,
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    #[test]
    fn decrypted_note_has_actual_incremental_path_and_spend_is_detected() {
        let fvk = key(1);
        let other = key(2);
        let a = action(
            &fvk,
            Nullifier::from_bytes(&[1; 32]).unwrap(),
            Scope::Internal,
        );
        let b = action(
            &other,
            Nullifier::from_bytes(&[2; 32]).unwrap(),
            Scope::External,
        );
        let mut scanner = Scanner::from_tree_state(&initial()).unwrap();
        let delta = scanner
            .apply(&block(vec![a, b], 4_134_001, 8, 9, 2), &fvk)
            .unwrap();
        assert_eq!(delta.observations.len(), 1);
        let (id, owned) = scanner.notes().iter().next().unwrap();
        assert_eq!(owned.scope, Scope::Internal);
        assert_eq!(
            owned.path().unwrap().root(owned.note.commitment().into()),
            scanner.anchor()
        );
        let spent_id = *id;
        let spend = action(
            &other,
            Nullifier::from_bytes(&owned.nullifier).unwrap(),
            Scope::External,
        );
        let delta = scanner
            .apply(&block(vec![spend], 4_134_002, 9, 10, 3), &fvk)
            .unwrap();
        assert_eq!(delta.spent_notes, vec![(spent_id, [7; 32])]);
        assert!(scanner.notes()[&spent_id].path().is_err());
        let mut bytes = Vec::new();
        write_commitment_tree(&scanner.tree, &mut bytes).unwrap();
        let state = TreeState {
            height: 4_134_002,
            hash: hex::encode([10; 32]),
            ironwood_tree: hex::encode(&bytes),
            ..initial()
        };
        scanner.verify_tree_state(&state).unwrap();
        let mut bad = state;
        bad.ironwood_tree.push_str("00");
        assert!(scanner.verify_tree_state(&bad).is_err());
    }
    #[test]
    fn malformed_late_action_or_wrong_tree_size_does_not_advance_scan() {
        let fvk = key(1);
        let good = action(
            &fvk,
            Nullifier::from_bytes(&[1; 32]).unwrap(),
            Scope::External,
        );
        let mut bad = good.clone();
        bad.ephemeral_key.truncate(31);
        let mut scanner = Scanner::from_tree_state(&initial()).unwrap();
        assert!(
            scanner
                .apply(&block(vec![good.clone(), bad], 4_134_001, 8, 9, 2), &fvk)
                .is_err()
        );
        assert_eq!(scanner.height(), 4_134_000);
        assert!(scanner.notes().is_empty());
        assert!(
            scanner
                .apply(&block(vec![good.clone()], 4_134_001, 8, 9, 2), &fvk)
                .is_err()
        );
        assert_eq!(scanner.height(), 4_134_000);
        assert!(scanner.notes().is_empty());
        assert!(
            scanner
                .apply(&block(vec![good.clone()], 4_134_001, 7, 9, 1), &fvk)
                .is_err()
        );
        scanner
            .apply(&block(vec![good], 4_134_001, 8, 9, 1), &fvk)
            .unwrap();
        assert_eq!(scanner.notes().len(), 1);
    }
    #[test]
    fn legacy_frontier_orphans_are_rejected_before_sdk_size_panics() {
        let leaf = MerkleHashOrchard::from_bytes(&[1; 32]).unwrap();
        for tree in [
            Tree::from_parts(None, Some(leaf), vec![]).unwrap(),
            Tree::from_parts(None, None, vec![Some(leaf)]).unwrap(),
        ] {
            let mut bytes = Vec::new();
            write_commitment_tree(&tree, &mut bytes).unwrap();
            let state = TreeState {
                ironwood_tree: hex::encode(bytes),
                ..initial()
            };
            assert!(Scanner::from_tree_state(&state).is_err());
        }
    }
}
