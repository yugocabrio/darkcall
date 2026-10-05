//! Funded local-regtest threshold spend gate. This is not auction settlement.
//! It consumes private wallet openings; the Python driver validates their chain
//! provenance and sends ONLY to the pinned loopback node with the regtest genesis.
use super::*;
use serde::Deserialize;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zcash_protocol::consensus::{BlockHeight, NetworkType};
const GENESIS: &str = "029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327";
#[derive(Clone, Copy)]
struct Regtest;
impl Parameters for Regtest {
    fn network_type(&self) -> NetworkType {
        NetworkType::Regtest
    }
    fn activation_height(&self, upgrade: NetworkUpgrade) -> Option<BlockHeight> {
        Some(BlockHeight::from_u32(match upgrade {
            NetworkUpgrade::Overwinter
            | NetworkUpgrade::Sapling
            | NetworkUpgrade::Blossom
            | NetworkUpgrade::Heartwood
            | NetworkUpgrade::Canopy => 1,
            NetworkUpgrade::Nu5
            | NetworkUpgrade::Nu6
            | NetworkUpgrade::Nu6_1
            | NetworkUpgrade::Nu6_2
            | NetworkUpgrade::Nu6_3 => 2,
        }))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Opening {
    cmx: [u8; 32],
    txid: [u8; 32],
    action_index: u32,
    mined_height: u32,
    mined_block: [u8; 32],
    recipient: Vec<u8>,
    amount: u64,
    rho: [u8; 32],
    rseed: [u8; 32],
    position: u32,
    auth_path: [[u8; 32]; 32],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WalletNotes {
    scope: String,
    genesis: String,
    nu6_3_height: u32,
    tip_height: u32,
    tip_hash: String,
    anchor: [u8; 32],
    notes: Vec<Opening>,
}
fn ignored(path: &Path) -> Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let path = path.canonicalize().map_err(|e| error("local path", e))?;
    if !path.starts_with(root.join(".local")) {
        return Err("regtest gate requires ignored local paths".into());
    }
    Ok(path)
}
fn read_private(path: &Path, limit: u64) -> Result<Vec<u8>> {
    ignored(path)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut f = options.open(path).map_err(|e| error("private input", e))?;
    let meta = f.metadata().map_err(|e| error("private metadata", e))?;
    if !meta.is_file() || meta.len() > limit {
        return Err("invalid private file shape".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.permissions().mode() & 0o077 != 0 || meta.nlink() != 1 {
            return Err("private input permissions/link count".into());
        }
    }
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)
        .map_err(|e| error("private read", e))?;
    Ok(bytes)
}
fn wallet_key(path: &Path) -> Result<SpendingKey> {
    let bytes: [u8; 32] = read_private(&path.join("private-spending-key.bin"), 32)?
        .try_into()
        .map_err(|_| "native key length")?;
    SpendingKey::from_bytes(bytes)
        .into_option()
        .ok_or("invalid native key".into())
}
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut f = options.open(path).map_err(|e| error("private output", e))?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| error("private output sync", e))
}
pub fn build(
    wallet: &Path,
    openings: &Path,
    recipient_wallet: &Path,
    name: &str,
) -> Result<serde_json::Value> {
    let start = Instant::now();
    if name.is_empty()
        || name.len() > 80
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("expected a new simple local run name".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let out = root.join(".local").join(name);
    if out.try_exists().map_err(|e| error("output path", e))? {
        return Err("regtest output already exists; choose a new run name".into());
    }
    let key = wallet_key(wallet)?;
    let fvk = FullViewingKey::from(&key);
    let receiver = FullViewingKey::from(&wallet_key(recipient_wallet)?);
    let state: WalletNotes = serde_json::from_slice(&read_private(openings, 1_000_000)?)
        .map_err(|e| error("private notes", e))?;
    if state.scope != "isolated regtest wallet openings"
        || state.genesis != GENESIS
        || state.nu6_3_height != 2
        || state.tip_height < 102
        || state.notes.is_empty()
        || state.notes.len() > 500
    {
        return Err("unsupported regtest funding state".into());
    }
    let opening = &state.notes[0];
    if state.tip_height
        < opening
            .mined_height
            .checked_add(100)
            .ok_or("maturity height overflow")?
    {
        return Err("regtest coinbase has not matured".into());
    }
    let recipient_bytes: [u8; 43] = opening
        .recipient
        .as_slice()
        .try_into()
        .map_err(|_| "recipient length")?;
    let recipient = orchard::Address::from_raw_address_bytes(&recipient_bytes)
        .into_option()
        .ok_or("invalid funding recipient")?;
    if recipient != fvk.address_at(0u32, Scope::External) {
        return Err("funding note belongs to another wallet".into());
    }
    let rho = Rho::from_bytes(&opening.rho)
        .into_option()
        .ok_or("noncanonical funding rho")?;
    let seed = RandomSeed::from_bytes(opening.rseed, &rho)
        .into_option()
        .ok_or("invalid funding rseed")?;
    let note = Note::from_parts(
        recipient,
        NoteValue::from_raw(opening.amount),
        rho,
        seed,
        NoteVersion::V3,
    )
    .into_option()
    .ok_or("invalid funding note")?;
    if ExtractedNoteCommitment::from(note.commitment()).to_bytes() != opening.cmx {
        return Err("funding opening differs from scanned cmx".into());
    }
    let nodes = opening.auth_path.each_ref().map(|b| {
        MerkleHashOrchard::from_bytes(b)
            .into_option()
            .ok_or("noncanonical Merkle element")
    });
    let nodes = nodes
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let path = MerklePath::from_parts(
        opening.position,
        nodes.try_into().map_err(|_| "path depth")?,
    );
    let anchor = path.root(note.commitment().into());
    if anchor.to_bytes() != state.anchor {
        return Err("funding witness differs from scanned canonical anchor".into());
    }
    let amount = 1_000_000u64;
    let fee = 10_000u64;
    let change = opening
        .amount
        .checked_sub(amount + fee)
        .ok_or("funding cannot pay transfer/fee")?;
    let height = BlockHeight::from_u32(
        state
            .tip_height
            .checked_add(1)
            .ok_or("target height overflow")?,
    );
    let mut rng = OsRng;
    let keys = split_key(&key, &mut rng)?;
    let mut builder = Builder::new(
        Regtest,
        height,
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
            ironwood_anchor: Some(anchor),
            orchard_pool_bundle_type: BundleType::DEFAULT,
        },
    );
    builder
        .add_ironwood_spend::<Infallible>(fvk.clone(), note, path)
        .map_err(|e| error("funded spend", e))?;
    for (address, value) in [
        (receiver.address_at(0u32, Scope::External), amount),
        (fvk.address_at(1u32, Scope::Internal), change),
    ] {
        builder
            .add_ironwood_output::<Infallible>(
                None,
                address,
                Zatoshis::from_u64(value).map_err(|e| error("value", e))?,
                MemoBytes::empty(),
            )
            .map_err(|e| error("funded output", e))?;
    }
    let parts = builder
        .build_for_pczt(rng, &FeeRule::standard())
        .map_err(|e| error("funded PCZT", e))?;
    let indices: [usize; 2] =
        std::array::from_fn(|i| parts.ironwood_meta.output_action_index(i).unwrap());
    let pczt = IoFinalizer::new(Creator::build_from_parts(parts.pczt_parts).ok_or("PCZT creator")?)
        .finalize_io()
        .map_err(|e| error("funded IO", e))?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|e| error("funded effects", e))?;
    if effects.version() != TxVersion::V6
        || effects.consensus_branch_id() != zcash_protocol::consensus::BranchId::Nu6_3
        || effects
            .ironwood_bundle()
            .ok_or("funded bundle")?
            .actions()
            .len()
            != 2
    {
        return Err("unexpected funded transaction shape".into());
    }
    let message: [u8; 32] = v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
    .try_into()
    .map_err(|_| "funded sighash length")?;
    let pczt = Verifier::new(pczt)
        .with_ironwood::<Infallible, _>(
            |bundle| -> std::result::Result<(), OrchardError<Infallible>> {
                bundle.verify_cross_address_restriction()?;
                for action in bundle.actions() {
                    action.verify_cv_net()?;
                    action.spend().verify_nullifier(Some(&fvk))?;
                    action.spend().verify_rk(Some(&fvk))?;
                    action.output().verify_note_commitment(action.spend())?;
                }
                Ok(())
            },
        )
        .map_err(|e| error("funded private fields", e))?
        .finish();
    let proof_start = Instant::now();
    let pk = ProvingKey::build(OrchardCircuitVersion::PostNu6_3);
    let pczt = Prover::new(pczt)
        .create_ironwood_proof(&pk)
        .map_err(|e| error("funded native proof", e))?
        .finish();
    let proof_seconds = proof_start.elapsed().as_secs_f64();
    let mut signed = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<SignError, _>(|_, bundle, _| {
            for action in bundle.actions_mut() {
                if action.spend().spend_auth_sig().is_none() {
                    let alpha = action
                        .spend()
                        .alpha()
                        .as_ref()
                        .ok_or_else(|| SignError::Custom("missing funded alpha".into()))?;
                    let signature = threshold_signature(&keys, alpha, &message, &mut rng)
                        .map_err(SignError::Custom)?;
                    action
                        .apply_signature(message, signature)
                        .map_err(SignError::Apply)?;
                    signed += 1;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .finish();
    if signed != 1 {
        return Err("expected one actual threshold spend".into());
    }
    let tx = TransactionExtractor::new(pczt)
        .extract()
        .map_err(|e| error("funded extraction", e))?;
    if tx.txid().as_ref() != &message {
        return Err("funded native-only txid mismatch".into());
    }
    let bundle = tx.ironwood_bundle().ok_or("funded final bundle")?;
    for (index, fvk, scope, expected) in [
        (indices[0], &receiver, Scope::External, amount),
        (indices[1], &fvk, Scope::Internal, change),
    ] {
        let (note, _, _) = bundle
            .decrypt_output_with_key(index, &fvk.to_ivk(scope))
            .ok_or("funded output decryption")?;
        if note.value().inner() != expected || note.version() != NoteVersion::V3 {
            return Err("funded output mismatch".into());
        }
    }
    fs::create_dir(&out).map_err(|e| error("funded output directory", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&out, fs::Permissions::from_mode(0o700))
            .map_err(|e| error("permissions", e))?;
    }
    let mut bytes = Vec::new();
    tx.write(&mut bytes)
        .map_err(|e| error("funded serialization", e))?;
    write_private(&out.join("regtest-transaction.bin"), &bytes)?;
    let provenance = serde_json::json!({"source_txid_le":opening.txid,"source_action":opening.action_index,
        "source_block_le":opening.mined_block,"source_height":opening.mined_height,"funding_tip_hash":state.tip_hash,
        "funding_tip_height":state.tip_height,"source_cmx":opening.cmx});
    write_private(
        &out.join("private-provenance.json"),
        &serde_json::to_vec(&provenance).map_err(|e| error("provenance", e))?,
    )?;
    let summary = serde_json::json!({"scope":"native isolated regtest threshold transfer; not auction settlement", "genesis":GENESIS,
        "inputs":1,"outputs":2,"threshold":"2-of-3","setup":"local trusted dealer; not DKG", "transaction_bytes":bytes.len(),
        "native_proof_and_signatures_verified":true,"recipients_decrypted":2,"planned_fee":fee,"recipient_amount":amount,
        "one_share_rejected":true,"actual_chain_provenance_checked_by_this_binary":false,"broadcast":false,
        "mpc_settlement_binding":false,"txid":tx.txid().to_string(),"proof_seconds":proof_seconds,"seconds":start.elapsed().as_secs_f64()});
    write_private(
        &out.join("summary.json"),
        &serde_json::to_vec_pretty(&summary).map_err(|e| error("summary", e))?,
    )?;
    Ok(summary)
}
