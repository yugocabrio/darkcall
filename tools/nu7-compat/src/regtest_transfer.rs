//! Actual NU7 regtest note -> native threshold transfer. No auction authorization
//! is claimed. Chain provenance/broadcast are checked by a separate loopback driver.
use crate::{
    Result, auction,
    interop_export::{create, write},
    native, regtest_wallet,
};
use orchard::{
    Note,
    circuit::{OrchardCircuitVersion, ProvingKey},
    keys::{FullViewingKey, Scope},
    note::{ExtractedNoteCommitment, NoteVersion, RandomSeed, Rho},
    tree::{MerkleHashOrchard, MerklePath},
    value::NoteValue,
};
use pczt::roles::{
    creator::Creator,
    io_finalizer::IoFinalizer,
    low_level_signer::Signer,
    prover::Prover,
    tx_extractor::TransactionExtractor,
    verifier::{OrchardError, Verifier},
};
use rand_old::rngs::OsRng;
use reddsa_frost::frost::redpallas as frost;
use serde::Deserialize;
use std::{collections::BTreeMap, convert::Infallible, path::Path, time::Instant};
use zcash_primitives::transaction::{
    TxVersion,
    builder::{BuildConfig, Builder, BundlePadding},
    fees::zip317::FeeRule,
    sighash::SignableInput,
    sighash_v6::v6_signature_hash,
    txid::TxIdDigester,
};
use zcash_protocol::{
    consensus::{BlockHeight, BranchId, NetworkType, NetworkUpgrade, Parameters},
    memo::MemoBytes,
    value::Zatoshis,
};

const GENESIS: &str = "029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327";
#[derive(Clone, Copy)]
pub(super) struct Regtest;
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
            | NetworkUpgrade::Nu6_3
            | NetworkUpgrade::Nu7 => 2,
        }))
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Opening {
    pub coinbase: bool,
    pub cmx: [u8; 32],
    pub txid: [u8; 32],
    pub action_index: u32,
    pub mined_height: u32,
    pub mined_block: [u8; 32],
    pub recipient: Vec<u8>,
    pub amount: u64,
    pub rho: [u8; 32],
    pub rseed: [u8; 32],
    pub position: u32,
    pub auth_path: [[u8; 32]; 32],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Notes {
    scope: String,
    genesis: String,
    branch: String,
    nu7_height: u32,
    tip_height: u32,
    tip_hash: String,
    anchor: [u8; 32],
    notes: Vec<Opening>,
}

pub(super) fn signing_keys(
    wallet: &Path,
    fvk: &FullViewingKey,
) -> Result<(native::ThresholdKeys, bool)> {
    if wallet.join("private-spending-key.bin").exists() {
        let bytes = auction::read(&wallet.join("private-spending-key.bin"), 32)?
            .try_into()
            .map_err(|_| "NU7 source key length")?;
        let key = orchard::keys::SpendingKey::from_bytes(bytes)
            .into_option()
            .ok_or("NU7 source key")?;
        return Ok((native::split_key(&key, &mut OsRng)?, false));
    }
    // The existing DKG initializer persists this exact pinned FROST encoding.
    // New and old native SDKs exchange FVK/scalar/signature bytes only.
    let public = frost::keys::PublicKeyPackage::deserialize(&auction::read(
        &wallet.join("private-frost-public-package.bin"),
        16384,
    )?)
    .map_err(|_| "NU7 DKG public package")?;
    if public
        .verifying_key()
        .serialize()
        .map_err(|_| "NU7 group key")?
        != fvk.to_bytes()[..32]
        || public.verifying_shares().len() != 3
    {
        return Err("NU7 DKG FVK/group/participant mismatch".into());
    }
    let mut packages = BTreeMap::new();
    for i in 0..3 {
        let id: frost::Identifier = ((i + 1) as u16)
            .try_into()
            .map_err(|_| "NU7 DKG identifier")?;
        let key = frost::keys::KeyPackage::deserialize(&auction::read(
            &wallet.join(format!("operator-{i}/private-frost-key-package.bin")),
            16384,
        )?)
        .map_err(|_| "NU7 DKG operator package")?;
        if key.identifier() != &id
            || key.min_signers() != &2
            || key.verifying_key() != public.verifying_key()
            || public.verifying_shares().get(&id) != Some(key.verifying_share())
            || frost::keys::VerifyingShare::from(*key.signing_share()) != *key.verifying_share()
        {
            return Err("NU7 DKG package group/share/threshold mismatch".into());
        }
        packages.insert(id, key);
    }
    Ok((native::ThresholdKeys { packages, public }, true))
}

pub(super) struct Source {
    pub index: usize,
    pub opening: Opening,
    pub tip_height: u32,
    pub tip_hash: String,
    pub fvk: FullViewingKey,
    pub note: Note,
    pub path: MerklePath,
}
pub(super) fn load_source(wallet: &Path, notes_path: &Path) -> Result<Source> {
    let wallet = auction::directory(wallet)?;
    auction::directory(notes_path.parent().ok_or("NU7 notes parent")?)?;
    let state: Notes = serde_json::from_slice(&auction::read(notes_path, 1_000_000)?)
        .map_err(|_| "NU7 actual notes schema")?;
    if state.scope != "isolated NU7 regtest wallet openings"
        || state.genesis != GENESIS
        || state.branch != "77190ad9"
        || state.nu7_height != 2
        || state.tip_height < 2
        || state.notes.is_empty()
        || state.notes.len() > 500
    {
        return Err("NU7 transfer requires actual isolated regtest notes".into());
    }
    let fvk = regtest_wallet::viewing_key(&wallet)?;
    let (index, source) = state
        .notes
        .iter()
        .enumerate()
        .find(|(_, n)| !n.coinbase || state.tip_height >= n.mined_height.saturating_add(100))
        .ok_or("NU7 source has no matured unspent note")?;
    let address = orchard::Address::from_raw_address_bytes(
        &source
            .recipient
            .as_slice()
            .try_into()
            .map_err(|_| "NU7 source receiver length")?,
    )
    .into_option()
    .ok_or("NU7 source receiver encoding")?;
    if address != fvk.address_at(0u32, Scope::External)
        && address != fvk.address_at(1u32, Scope::Internal)
    {
        return Err("NU7 source note belongs to another wallet".into());
    }
    let rho = Rho::from_bytes(&source.rho)
        .into_option()
        .ok_or("NU7 source rho")?;
    let seed = RandomSeed::from_bytes(source.rseed, &rho)
        .into_option()
        .ok_or("NU7 source seed")?;
    let note = Note::from_parts(
        address,
        NoteValue::from_raw(source.amount),
        rho,
        seed,
        NoteVersion::V3,
    )
    .into_option()
    .ok_or("NU7 source note")?;
    if ExtractedNoteCommitment::from(note.commitment()).to_bytes() != source.cmx {
        return Err("NU7 source opening differs from canonical scanned commitment".into());
    }
    let nodes = source
        .auth_path
        .iter()
        .map(|b| {
            MerkleHashOrchard::from_bytes(b)
                .into_option()
                .ok_or("NU7 source Merkle element")
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let path = MerklePath::from_parts(
        source.position,
        nodes.try_into().map_err(|_| "NU7 path depth")?,
    );
    let anchor = path.root(note.commitment().into());
    if anchor.to_bytes() != state.anchor {
        return Err("NU7 source witness differs from node anchor".into());
    }
    Ok(Source {
        index,
        opening: source.clone(),
        tip_height: state.tip_height,
        tip_hash: state.tip_hash,
        fvk,
        note,
        path,
    })
}

pub fn build(
    wallet: &Path,
    notes_path: &Path,
    recipient: &Path,
    name: &str,
) -> Result<serde_json::Value> {
    let started = Instant::now();
    let wallet = auction::directory(wallet)?;
    let loaded = load_source(&wallet, notes_path)?;
    let Source {
        index,
        opening,
        tip_height,
        tip_hash,
        fvk,
        note,
        path,
    } = loaded;
    let source = &opening;
    let receiver = regtest_wallet::viewing_key(recipient)?;
    let anchor = path.root(note.commitment().into());
    let (keys, dkg) = signing_keys(&wallet, &fvk)?;
    let fee = 10_000u64;
    let amount = (source.amount / 2).min(1_000_000u64);
    let change = source
        .amount
        .checked_sub(amount + fee)
        .ok_or("NU7 source cannot cover transfer/fee")?;
    let height = BlockHeight::from_u32(tip_height.checked_add(1).ok_or("NU7 height overflow")?);
    let mut rng = rand::rng();
    let mut old_rng = OsRng;
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
    builder
        .add_ironwood_spend::<Infallible>(fvk.clone(), note, path)
        .map_err(|_| "NU7 actual spend build")?;
    for (address, value) in [
        (receiver.address_at(0u32, Scope::External), amount),
        (fvk.address_at(1u32, Scope::Internal), change),
    ] {
        builder
            .add_ironwood_output::<Infallible>(
                None,
                address,
                Zatoshis::from_u64(value).map_err(|_| "NU7 transfer money range")?,
                MemoBytes::empty(),
            )
            .map_err(|_| "NU7 transfer output")?;
    }
    let parts = builder
        .build_for_pczt(&mut rng, &FeeRule::standard())
        .map_err(|_| "NU7 funded PCZT")?;
    let indices: [usize; 2] =
        std::array::from_fn(|i| parts.ironwood_meta.output_action_index(i).unwrap());
    let pczt = IoFinalizer::new(Creator::build_from_parts(parts.pczt_parts).ok_or("NU7 creator")?)
        .finalize_io(&mut rng)
        .map_err(|_| "NU7 funded IO")?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|_| "NU7 funded effects")?;
    if effects.version() != TxVersion::V6
        || effects.consensus_branch_id() != BranchId::Nu7
        || effects.transparent_bundle().is_some()
        || effects.sapling_bundle().is_some()
        || effects.orchard_bundle().is_some()
        || effects
            .ironwood_bundle()
            .ok_or("NU7 funded bundle")?
            .actions()
            .len()
            != 2
    {
        return Err("NU7 funded shape/branch mismatch".into());
    }
    let message: [u8; 32] = v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
    .try_into()
    .map_err(|_| "NU7 funded sighash length")?;
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
        .map_err(|_| "NU7 funded private-field verification")?
        .finish();
    let proof_start = Instant::now();
    let pk = ProvingKey::build(OrchardCircuitVersion::PostNu6_3);
    let pczt = Prover::new(pczt)
        .create_ironwood_proof(&mut rng, &pk)
        .map_err(|_| "NU7 funded proof")?
        .finish();
    let proof_seconds = proof_start.elapsed().as_secs_f64();
    let mut spends = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<native::SignError, _>(|_, bundle, _| {
            for action in bundle.actions_mut() {
                if action.spend().spend_auth_sig().is_none() {
                    let alpha = action
                        .spend()
                        .alpha()
                        .as_ref()
                        .ok_or_else(|| native::SignError::Custom("NU7 alpha".into()))?;
                    let signature =
                        native::threshold_signature(&keys, alpha, &message, &mut old_rng)
                            .map_err(native::SignError::Custom)?;
                    let mut changed = message;
                    changed[0] ^= 1;
                    if action.spend().rk().verify(&changed, &signature).is_ok() {
                        return Err(native::SignError::Custom(
                            "NU7 changed digest accepted".into(),
                        ));
                    }
                    action
                        .apply_signature(message, signature)
                        .map_err(native::SignError::Apply)?;
                    spends += 1;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .finish();
    if spends != 1 {
        return Err("NU7 expected one actual threshold spend".into());
    }
    let tx = TransactionExtractor::new(pczt)
        .extract(&mut rng)
        .map_err(|_| "NU7 funded extraction")?;
    if tx.txid().as_ref() != &message {
        return Err("NU7 funded effects identity changed".into());
    }
    let bundle = tx.ironwood_bundle().ok_or("NU7 final bundle")?;
    for (i, key, scope, value) in [
        (indices[0], &receiver, Scope::External, amount),
        (indices[1], &fvk, Scope::Internal, change),
    ] {
        let (note, address, memo) = bundle
            .decrypt_output_with_key(i, &key.to_ivk(scope))
            .ok_or("NU7 funded output decryption")?;
        let expected_address =
            key.address_at(if scope == Scope::External { 0u32 } else { 1u32 }, scope);
        if note.value().inner() != value
            || note.version() != NoteVersion::V3
            || address != expected_address
            || memo != *MemoBytes::empty().as_array()
        {
            return Err("NU7 funded output substituted".into());
        }
    }
    let mut bytes = Vec::new();
    tx.write(&mut bytes).map_err(|_| "NU7 funded encoding")?;
    native::verify_bytes(&bytes, BranchId::Nu7, &mut rng)?;
    let output = create(name)?;
    write(&output.join("nu7-regtest-transaction.bin"), &bytes)?;
    write(&output.join("private-provenance.json"),&serde_json::to_vec(&serde_json::json!({
        "source_index":index,"source_txid":source.txid,"source_action":source.action_index,
        "source_cmx":source.cmx,"source_height":source.mined_height,"source_block":source.mined_block,
        "funding_tip_height":tip_height,"funding_tip_hash":tip_hash})).map_err(|_| "NU7 provenance encoding")?)?;
    let result = serde_json::json!({"scope":"actual-note isolated NU7 threshold transfer; not auction settlement",
        "genesis":GENESIS,"branch":"77190ad9","inputs":1,"outputs":2,"threshold":"2-of-3",
        "setup":if dkg {"existing local DKG packages; all shares on one host"} else {"ordinary bank key, local trusted dealer"},
        "dkg_source":dkg,"source_master_key_read":!dkg,"native_proof_and_signatures_verified":true,
        "one_share_rejected":true,"changed_digest_rejected":true,"recipients_decrypted":2,
        "recipient_amount":amount,"planned_fee":fee,"txid":tx.txid().to_string(),"transaction_bytes":bytes.len(),
        "proof_seconds":proof_seconds,"seconds":started.elapsed().as_secs_f64(),"broadcast":false,
        "chain_provenance_checked_by_this_binary":false,"mpc_settlement_binding":false});
    write(
        &output.join("summary.json"),
        &serde_json::to_vec(&result).map_err(|_| "NU7 transfer summary")?,
    )?;
    Ok(result)
}
