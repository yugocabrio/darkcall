//! NU7 six-output synthetic auction adapter. The working proof process and this
//! SDK exchange canonical bytes only. Fixed application proofs are verified
//! against the in-memory PCZT effects before any threshold spend signature.
use crate::{
    Result,
    interop_export::{SDK, write},
    native,
};
use incrementalmerkletree::Hashable;
use orchard::{
    circuit::{OrchardCircuitVersion, ProvingKey},
    keys::{FullViewingKey, Scope, SpendingKey},
    note::{ExtractedNoteCommitment, NoteVersion},
    tree::{MerkleHashOrchard, MerklePath},
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
    convert::Infallible,
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
    consensus::{BranchId, NetworkUpgrade, Parameters, TEST_NETWORK},
    memo::MemoBytes,
    value::Zatoshis,
};

pub const SCOPE: &str = "synthetic NU7 six-output auction; no chain funding";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: u32,
    scope: String,
    sdk: String,
    branch: String,
    amounts: [u64; 6],
    #[serde(default)]
    receipt_memos: bool,
}

fn receipt_memos(directory: &Path, request: &Request, linked: bool) -> Result<[MemoBytes; 6]> {
    let mut memos = std::array::from_fn(|_| MemoBytes::empty());
    if request.receipt_memos {
        // Independent fixed C++ proof + canonical ticket opening verification.
        // Finalization additionally links this statement to the six-row auction.
        let mut command = Command::new("python3");
        command
            .arg(root().join("scripts/check_auction_memos.py"))
            .arg(directory);
        if linked {
            command.arg("--linked-settlement");
        }
        let output = command
            .output()
            .map_err(|_| "receipt proof verifier launch")?;
        if !output.status.success() || output.stdout.len() > 4096 {
            return Err("auction receipt proof/opening rejected before FROST".into());
        }
        for (i, memo) in memos.iter_mut().enumerate().take(4) {
            *memo = MemoBytes::from_bytes(&read(&directory.join(format!("ticket-{i}.memo")), 512)?)
                .map_err(|_| "auction memo length")?;
        }
    }
    Ok(memos)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Keys {
    format: u32,
    spend: [u8; 32],
    receivers: Vec<Vec<u8>>,
    indices: [u32; 6],
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
    native_actions: [[u8; 32]; 6],
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}
pub(crate) fn directory(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .map_err(|_| "missing NU7 auction directory")?;
    if !path.starts_with(
        root()
            .join(".local")
            .canonicalize()
            .map_err(|_| "local directory")?,
    ) || !path.is_dir()
    {
        return Err("NU7 auction accepts ignored synthetic artifacts only".into());
    }
    Ok(path)
}
pub(crate) fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "NU7 auction bounded artifact unavailable")?;
    let meta = file.metadata().map_err(|_| "NU7 artifact metadata")?;
    if !meta.is_file() || meta.mode() & 0o077 != 0 || meta.nlink() != 1 || meta.len() > limit {
        return Err("NU7 artifact type, permissions, links or size differ".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "NU7 artifact read")?;
    if bytes.len() as u64 > limit {
        return Err("NU7 artifact exceeds bound".into());
    }
    Ok(bytes)
}
fn json<T: serde::de::DeserializeOwned>(directory: &Path, name: &str) -> Result<T> {
    serde_json::from_slice(&read(&directory.join(name), 65536)?)
        .map_err(|_| "strict NU7 auction schema rejected".into())
}
fn write_json(directory: &Path, name: &str, value: &impl Serialize) -> Result<()> {
    write(
        &directory.join(name),
        &serde_json::to_vec(value).map_err(|_| "NU7 encoding")?,
    )
}
fn request(directory: &Path) -> Result<Request> {
    let value: Request = json(directory, "nu7-auction-request.json")?;
    if value.format != 1
        || value.scope != SCOPE
        || value.sdk != SDK
        || value.branch != "77190ad9"
        || value.amounts.iter().map(|v| u128::from(*v)).sum::<u128>() + 30000 != 4 * 15100
    {
        return Err("NU7 auction request identity or value conservation".into());
    }
    Ok(value)
}
fn identity<A: Authorization>(tx: &TransactionData<A>) -> Result<Identity> {
    if tx.version() != TxVersion::V6
        || tx.consensus_branch_id() != BranchId::Nu7
        || tx.transparent_bundle().is_some()
        || tx.sapling_bundle().is_some()
        || tx.orchard_bundle().is_some()
    {
        return Err("NU7 auction transaction pools/version/branch".into());
    }
    let native_actions = tx
        .ironwood_bundle()
        .ok_or("NU7 native bundle")?
        .actions()
        .iter()
        .map(|a| a.cmx().to_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "NU7 expects six actions")?;
    // Only the explicitly rejected-other-pools shape permits this shortcut.
    // Preparation/finalization also check the SDK's full signature hash below.
    let txid = to_txid(
        tx.version(),
        tx.consensus_branch_id(),
        &tx.digest(TxIdDigester),
    );
    let effects_digest = *txid.as_ref();
    Ok(Identity {
        format: 1,
        scope: SCOPE.into(),
        sdk: SDK.into(),
        branch: "77190ad9".into(),
        txid: txid.to_string(),
        effects_digest,
        native_actions,
    })
}
fn application(directory: &Path, actual: &Identity) -> Result<serde_json::Value> {
    // The trusted verifier is our compiled executable, never supplied in a bundle.
    // The expected effects come directly from the SDK's in-memory native object.
    let mut child = Command::new(root().join("target/release/darkcall-native-gate"))
        .arg("--verify-nu7-auction-application")
        .arg(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| "fixed application verifier launch")?;
    child
        .stdin
        .take()
        .ok_or("verifier stdin")?
        .write_all(&serde_json::to_vec(actual).map_err(|_| "identity encoding")?)
        .map_err(|_| "expected effects transfer")?;
    let output = child
        .wait_with_output()
        .map_err(|_| "fixed verifier wait")?;
    if !output.status.success() {
        return Err("NU7 application proof rejected before FROST signing".into());
    }
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|_| "fixed verifier summary")?;
    if result["native_row_proofs_verified"] != true
        || result["normal_cpp_auction_proof_verified"] != true
    {
        return Err("incomplete NU7 application verification".into());
    }
    Ok(result)
}

pub fn prepare(path: &Path) -> Result<serde_json::Value> {
    let directory = directory(path)?;
    let request = request(&directory)?;
    let memos = receipt_memos(&directory, &request, false)?;
    let mut rng = rand::rng();
    let sk = native::random_key(&mut rng);
    let fvk = FullViewingKey::from(&sk);
    let notes: [_; 4] = std::array::from_fn(|_| native::synthetic_note(&fvk, 15100, &mut rng));
    let leaves = notes
        .each_ref()
        .map(|n| MerkleHashOrchard::from_cmx(&n.commitment().into()));
    let pairs: [_; 2] = std::array::from_fn(|i| {
        MerkleHashOrchard::combine(0.into(), &leaves[2 * i], &leaves[2 * i + 1])
    });
    let paths: [_; 4] = std::array::from_fn(|i| {
        MerklePath::from_parts(
            i as u32,
            std::array::from_fn(|level| match level {
                0 => leaves[i ^ 1],
                1 => pairs[(i / 2) ^ 1],
                _ => MerkleHashOrchard::empty_root((level as u8).into()),
            }),
        )
    });
    let anchor = paths[0].root(notes[0].commitment().into());
    let height = TEST_NETWORK
        .activation_height(NetworkUpgrade::Nu7)
        .ok_or("NU7 height")?
        + 1;
    let mut builder = Builder::new(
        TEST_NETWORK,
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
    for (note, path) in notes.into_iter().zip(paths) {
        if path.root(note.commitment().into()) != anchor {
            return Err("fabricated roots differ".into());
        }
        builder
            .add_ironwood_spend::<Infallible>(fvk.clone(), note, path)
            .map_err(|_| "synthetic spend build")?;
    }
    let receivers: [_; 6] =
        std::array::from_fn(|_| FullViewingKey::from(&native::random_key(&mut rng)));
    for (i, (receiver, amount)) in receivers.iter().zip(request.amounts).enumerate() {
        builder
            .add_ironwood_output::<Infallible>(
                None,
                receiver.address_at(0u32, Scope::External),
                Zatoshis::from_u64(amount).map_err(|_| "output money range")?,
                memos[i].clone(),
            )
            .map_err(|_| "NU7 output build")?;
    }
    let parts = builder
        .build_for_pczt(&mut rng, &FeeRule::standard())
        .map_err(|_| "NU7 PCZT build")?;
    let indices: [_; 6] =
        std::array::from_fn(|i| parts.ironwood_meta.output_action_index(i).unwrap() as u32);
    let pczt = IoFinalizer::new(Creator::build_from_parts(parts.pczt_parts).ok_or("PCZT version")?)
        .finalize_io(&mut rng)
        .map_err(|_| "NU7 IO finalize")?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|_| "NU7 PCZT effects")?;
    let actual = identity(&effects)?;
    if v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
        != actual.effects_digest
    {
        return Err("NU7 native-only effects/sighash mismatch".into());
    }
    let bundle = effects.ironwood_bundle().ok_or("Ironwood effects")?;
    let mut openings = Vec::new();
    for (i, receiver) in receivers.iter().enumerate() {
        let (note, address, memo) = bundle
            .decrypt_output_with_key(indices[i] as usize, &receiver.to_ivk(Scope::External))
            .ok_or("constructed recipient cannot decrypt")?;
        if note.value().inner() != request.amounts[i]
            || address != receiver.address_at(0u32, Scope::External)
            || note.version() != NoteVersion::V3
            || memo != *memos[i].as_array()
            || ExtractedNoteCommitment::from(note.commitment())
                != *bundle.actions()[indices[i] as usize].cmx()
        {
            return Err("NU7 prepared note mismatch".into());
        }
        openings.push(serde_json::json!({"action_index":indices[i], "recipient":address.to_raw_address_bytes().to_vec(),
            "amount":note.value().inner(),"rho":note.rho().to_bytes(),"rseed":note.rseed().as_bytes(),"cmx":actual.native_actions[indices[i] as usize]}));
    }
    write_json(&directory, "nu7-native-identity.json", &actual)?;
    write_json(
        &directory,
        "private-nu7-auction-outputs.json",
        &serde_json::json!({"format":1,"scope":SCOPE,"sdk":SDK,
        "branch":"77190ad9","txid":actual.txid,"outputs":openings}),
    )?;
    write_json(
        &directory,
        "private-nu7-auction-keys.json",
        &Keys {
            format: 1,
            spend: *sk.to_bytes(),
            receivers: receivers.iter().map(|r| r.to_bytes().to_vec()).collect(),
            indices,
        },
    )?;
    write(
        &directory.join("nu7-prepared.pczt"),
        &pczt.serialize().map_err(|_| "NU7 PCZT encoding")?,
    )?;
    Ok(
        serde_json::json!({"prepared":true,"actions":6,"synthetic_inputs":4,"fee":30000,
        "recipients_decrypted_before_proving":6,"receipt_memos_proof_authenticated":request.receipt_memos,
        "refund_memos_in_same_pczt":if request.receipt_memos {4} else {0},
        "seller_operator_memos_empty":true,"threshold_signatures_created":false,"broadcast":false}),
    )
}

pub fn finalize(path: &Path) -> Result<serde_json::Value> {
    let start = Instant::now();
    let directory = directory(path)?;
    let request = request(&directory)?;
    let memos = receipt_memos(&directory, &request, false)?;
    let keys: Keys = json(&directory, "private-nu7-auction-keys.json")?;
    if keys.format != 1 || keys.receivers.len() != 6 {
        return Err("NU7 synthetic keys shape".into());
    }
    let sk = SpendingKey::from_bytes(keys.spend)
        .into_option()
        .ok_or("synthetic key encoding")?;
    let fvk = FullViewingKey::from(&sk);
    let receivers = keys
        .receivers
        .iter()
        .map(|v| {
            FullViewingKey::from_bytes(v.as_slice().try_into().map_err(|_| "receiver FVK length")?)
                .ok_or("receiver FVK encoding")
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let encoded = read(&directory.join("nu7-prepared.pczt"), 1_000_000)?;
    let pczt = Pczt::parse(&encoded).map_err(|_| "NU7 PCZT parse")?;
    // Upstream's decoder accepts trailing bytes. Require our own exact canonical
    // serialization before interpreting a persisted synthetic signing request.
    if pczt
        .clone()
        .serialize()
        .map_err(|_| "NU7 PCZT serialization")?
        != encoded
    {
        return Err("NU7 PCZT parse: noncanonical or trailing data".into());
    }
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|_| "NU7 prepared effects")?;
    let actual = identity(&effects)?;
    if v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
        != actual.effects_digest
    {
        return Err("NU7 prepared effects/sighash mismatch".into());
    }
    let saved: Identity = json(&directory, "nu7-native-identity.json")?;
    if actual != saved {
        return Err("NU7 prepared effects changed".into());
    }
    let bundle = effects.ironwood_bundle().ok_or("Ironwood missing")?;
    let mut used = std::collections::BTreeSet::new();
    for (i, receiver) in receivers.iter().enumerate() {
        let index = keys.indices[i] as usize;
        if index >= 6 || !used.insert(index) {
            return Err("NU7 recipient index collision".into());
        }
        let (note, address, memo) = bundle
            .decrypt_output_with_key(index, &receiver.to_ivk(Scope::External))
            .ok_or("prepared output cannot decrypt")?;
        if note.value().inner() != request.amounts[i]
            || address != receiver.address_at(0u32, Scope::External)
            || memo != *memos[i].as_array()
        {
            return Err("NU7 prepared output substituted".into());
        }
    }
    // Application proof is checked on exact in-memory effects BEFORE signing.
    eprintln!("NU7 fixed six-row/MPC application verification before FROST started");
    let app = application(&directory, &actual)?;
    receipt_memos(&directory, &request, true)?;
    let mut before_fvks = Vec::new();
    let pczt = Verifier::new(pczt)
        .with_ironwood::<Infallible, _>(
            |bundle| -> std::result::Result<(), OrchardError<Infallible>> {
                before_fvks = bundle
                    .actions()
                    .iter()
                    .map(|a| a.spend().fvk().as_ref().map(FullViewingKey::to_bytes))
                    .collect();
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
        .map_err(|_| "NU7 private fields rejected")?
        .finish();
    let mut rng = rand::rng();
    let mut old_rng = rand_old::rngs::OsRng;
    let threshold = native::split_key(&sk, &mut old_rng)?;
    let proof_start = Instant::now();
    let pk = ProvingKey::build(OrchardCircuitVersion::PostNu6_3);
    let pczt = Prover::new(pczt)
        .create_ironwood_proof(&mut rng, &pk)
        .map_err(|_| "NU7 native proof")?
        .finish();
    let proof_seconds = proof_start.elapsed().as_secs_f64();
    if identity(
        &pczt
            .clone()
            .into_effects()
            .map_err(|_| "pre-sign effects")?,
    )? != actual
    {
        return Err("NU7 effects changed before signing".into());
    }
    let mut spends = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<native::SignError, _>(|_, bundle, _| {
            for action in bundle.actions_mut() {
                if action.spend().spend_auth_sig().is_none() {
                    let alpha = action
                        .spend()
                        .alpha()
                        .as_ref()
                        .ok_or_else(|| native::SignError::Custom("alpha missing".into()))?;
                    let signature = native::threshold_signature(
                        &threshold,
                        alpha,
                        &actual.effects_digest,
                        &mut old_rng,
                    )
                    .map_err(native::SignError::Custom)?;
                    let mut changed = actual.effects_digest;
                    changed[0] ^= 1;
                    if action.spend().rk().verify(&changed, &signature).is_ok() {
                        return Err(native::SignError::Custom("changed digest accepted".into()));
                    }
                    action
                        .apply_signature(actual.effects_digest, signature)
                        .map_err(native::SignError::Apply)?;
                    spends += 1;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .finish();
    if spends != 4 {
        return Err("NU7 expects four threshold spends".into());
    }
    let pczt = Verifier::new(pczt)
        .with_ironwood::<String, _>(|bundle| -> std::result::Result<(), OrchardError<String>> {
            let after: Vec<_> = bundle
                .actions()
                .iter()
                .map(|a| a.spend().fvk().as_ref().map(FullViewingKey::to_bytes))
                .collect();
            if after != before_fvks {
                return Err(OrchardError::Custom("NU7 signer changed FVK bytes".into()));
            }
            Ok(())
        })
        .map_err(|_| "NU7 signer FVK preservation")?
        .finish();
    let tx = TransactionExtractor::new(pczt)
        .extract(&mut rng)
        .map_err(|_| "NU7 extract/native verification")?;
    if identity(&tx)? != actual {
        return Err("NU7 extracted effects changed".into());
    }
    for (i, receiver) in receivers.iter().enumerate() {
        let (note, address, memo) = tx
            .ironwood_bundle()
            .ok_or("Ironwood missing")?
            .decrypt_output_with_key(keys.indices[i] as usize, &receiver.to_ivk(Scope::External))
            .ok_or("final recipient cannot decrypt")?;
        if note.value().inner() != request.amounts[i]
            || address != receiver.address_at(0u32, Scope::External)
            || note.version() != NoteVersion::V3
            || memo != *memos[i].as_array()
        {
            return Err("NU7 final recipient mismatch".into());
        }
    }
    let mut bytes = Vec::new();
    tx.write(&mut bytes).map_err(|_| "NU7 serialization")?;
    native::verify_bytes(&bytes, BranchId::Nu7, &mut rng)?;
    write(&directory.join("synthetic-nu7-transaction.bin"), &bytes)?;
    if request.receipt_memos {
        crate::receipt_transport::persist_auction(
            &directory,
            &tx.txid().to_string(),
            &receivers,
            &keys.indices,
            &request.amounts,
        )?;
    }
    Ok(
        serde_json::json!({"scope":SCOPE,"branch":"77190ad9","version":6,"synthetic_inputs":4,"actions":6,
        "threshold_spends":spends,"threshold":"2-of-3","setup":"local trusted dealer; no DKG",
        "auction_proofs_verified_before_signing":true,"application":app,"recipients_decrypted":6,
        "native_proof_and_signatures_verified":true,"changed_sighash_rejected":true,"one_share_rejected":true,
        "pczt_fvks_preserved":true,
        "four_receipt_memos_in_same_six_output_transaction":request.receipt_memos,
        "txid":tx.txid().to_string(),"fee":30000,"transaction_bytes":bytes.len(),"proof_seconds":proof_seconds,
        "seconds":start.elapsed().as_secs_f64(),"chain_funding":false,"broadcast":false}),
    )
}
pub fn verify(path: &Path) -> Result<serde_json::Value> {
    let start = Instant::now();
    let directory = directory(path)?;
    let bytes = read(&directory.join("synthetic-nu7-transaction.bin"), 100000)?;
    let tx = native::verify_bytes(&bytes, BranchId::Nu7, &mut rand::rng())?;
    let actual = identity(&tx)?;
    let saved: Identity = json(&directory, "nu7-native-identity.json")?;
    if saved != actual {
        return Err("NU7 public effects identity mismatch".into());
    }
    let app = application(&directory, &actual)?;
    Ok(
        serde_json::json!({"scope":"public-only NU7 six-output native/MPC/row replay; synthetic funding",
        "native_proof_and_signatures_verified":true,"application":app,"private_openings_keys_pczt_read":false,
        "actual_transaction_outputs_match":true,"txid":tx.txid().to_string(),"transaction_bytes":bytes.len(),
        "seconds":start.elapsed().as_secs_f64(),"chain_funding_or_inclusion_verified":false,"broadcast":false}),
    )
}
