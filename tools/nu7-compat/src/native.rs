//! NU7 native PCZT/FROST compatibility using fabricated funding only.
use crate::Result;
use ff::{Field, PrimeField};
use incrementalmerkletree::Hashable;
use orchard::{
    Note,
    circuit::{OrchardCircuitVersion, ProvingKey, VerifyingKey},
    keys::{FullViewingKey, Scope, SpendAuthorizingKey, SpendValidatingKey, SpendingKey},
    note::{ExtractedNoteCommitment, NoteVersion, RandomSeed, Rho},
    primitives::redpallas::{Signature, SpendAuth},
    tree::{MerkleHashOrchard, MerklePath},
};
use pasta_curves::pallas;
use pczt::roles::{
    creator::Creator,
    io_finalizer::IoFinalizer,
    low_level_signer::{OrchardParseError, Signer},
    prover::Prover,
    tx_extractor::TransactionExtractor,
    verifier::{OrchardError, Verifier},
};
use rand_old::rngs::OsRng as OldOsRng;
use reddsa_frost::frost::redpallas as frost;
use std::{collections::BTreeMap, convert::Infallible, time::Instant};
use zcash_primitives::transaction::{
    Transaction, TxVersion,
    builder::{BuildConfig, Builder, BundlePadding},
    fees::zip317::FeeRule,
    sighash::SignableInput,
    sighash_v6::v6_signature_hash,
    txid::TxIdDigester,
};
use zcash_protocol::{
    consensus::{BranchId, NetworkUpgrade, Parameters, TEST_NETWORK},
    memo::MemoBytes,
    value::Zatoshis,
};
fn error(context: &str, e: impl std::fmt::Debug) -> String {
    format!("{context}: {e:?}")
}
pub(crate) fn random_key(rng: &mut impl rand::CryptoRng) -> SpendingKey {
    loop {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        if let Some(k) = SpendingKey::from_bytes(bytes).into_option() {
            return k;
        }
    }
}
pub(crate) fn synthetic_note(
    fvk: &FullViewingKey,
    value: u64,
    rng: &mut impl rand::CryptoRng,
) -> Note {
    loop {
        let rho = Rho::from_bytes(&pallas::Base::random(&mut *rng).to_repr()).unwrap();
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        if let Some(seed) = RandomSeed::from_bytes(bytes, &rho).into_option()
            && let Some(n) = Note::from_parts(
                fvk.address_at(0u32, Scope::External),
                orchard::value::NoteValue::from_raw(value),
                rho,
                seed,
                NoteVersion::V3,
            )
            .into_option()
        {
            return n;
        }
    }
}
pub(crate) struct ThresholdKeys {
    packages: BTreeMap<frost::Identifier, frost::keys::KeyPackage>,
    public: frost::keys::PublicKeyPackage,
}

pub(crate) fn split_key(sk: &SpendingKey, rng: &mut OldOsRng) -> Result<ThresholdKeys> {
    // Local test trusted-dealer setup only. Deployment must use a reviewed setup.
    let ask = SpendAuthorizingKey::from(sk);
    let bytes: [u8; 32] = ask.randomize(&pallas::Scalar::zero()).to_bytes();
    let signing_key = frost::SigningKey::deserialize(&bytes).map_err(|e| error("FROST key", e))?;
    let (shares, public) = frost::keys::split(
        &signing_key,
        3,
        2,
        frost::keys::IdentifierList::Default,
        rng,
    )
    .map_err(|e| error("split", e))?;
    if public
        .verifying_key()
        .serialize()
        .map_err(|e| error("public key", e))?
        != SpendValidatingKey::from(&ask).to_bytes()
    {
        return Err("FROST group key differs from the note key".into());
    }
    let packages = shares
        .into_iter()
        .map(|(id, share)| frost::keys::KeyPackage::try_from(share).map(|key| (id, key)))
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| error("key package", e))?;
    Ok(ThresholdKeys { packages, public })
}

pub(crate) fn threshold_signature(
    keys: &ThresholdKeys,
    alpha: &pallas::Scalar,
    message: &[u8; 32],
    rng: &mut OldOsRng,
) -> Result<Signature<SpendAuth>> {
    let mut nonces = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    // Exactly two of the three key shares; fresh nonces for every action.
    for (id, key) in keys.packages.iter().take(2) {
        let (nonce, commitment) = frost::round1::commit(key.signing_share(), &mut *rng);
        nonces.insert(*id, nonce);
        commitments.insert(*id, commitment);
    }
    let package = frost::SigningPackage::new(commitments, message);
    let randomizer =
        frost::Randomizer::deserialize(&alpha.to_repr()).map_err(|e| error("randomizer", e))?;
    let params = frost::RandomizedParams::from_randomizer(keys.public.verifying_key(), randomizer);
    let mut signatures = BTreeMap::new();
    for (id, nonce) in nonces {
        let share = frost::round2::sign(&package, &nonce, &keys.packages[&id], randomizer)
            .map_err(|e| error("signature share", e))?;
        signatures.insert(id, share);
    }
    let one = signatures.iter().take(1).map(|(id, s)| (*id, *s)).collect();
    if frost::aggregate(&package, &one, &keys.public, &params).is_ok() {
        return Err("single share unexpectedly authorized 2-of-3 spend".into());
    }
    let signature = frost::aggregate(&package, &signatures, &keys.public, &params)
        .map_err(|e| error("aggregate", e))?;
    let bytes: [u8; 64] = signature
        .serialize()
        .map_err(|e| error("signature bytes", e))?
        .try_into()
        .map_err(|_| "signature length")?;
    Ok(Signature::from(bytes))
}

pub(crate) fn verify_bytes(
    bytes: &[u8],
    branch: BranchId,
    rng: &mut impl rand::CryptoRng,
) -> Result<Transaction> {
    let mut reader = std::io::Cursor::new(bytes);
    let tx = Transaction::read(&mut reader, branch).map_err(|e| error("native parse", e))?;
    if reader.position() != bytes.len() as u64
        || tx.version() != TxVersion::V6
        || tx.consensus_branch_id() != branch
        || tx.transparent_bundle().is_some()
        || tx.sapling_bundle().is_some()
        || tx.orchard_bundle().is_some()
    {
        return Err("unexpected native transaction branch, shape or trailing data".into());
    }
    let bundle = tx.ironwood_bundle().ok_or("missing Ironwood bundle")?;
    let vk = VerifyingKey::build(OrchardCircuitVersion::PostNu6_3);
    let mut validator = orchard::bundle::BatchValidator::new(&vk);
    validator
        .add_bundle(bundle, *tx.txid().as_ref())
        .map_err(|e| error("proof/signature batch", e))?;
    if !validator.validate(rng) {
        return Err("native proof or signatures invalid".into());
    }
    Ok(tx)
}

pub fn verify_old(path: &str) -> Result<serde_json::Value> {
    let started = Instant::now();
    let metadata = std::fs::metadata(path).map_err(|e| error("public transaction metadata", e))?;
    if !metadata.is_file() || metadata.len() > 1_000_000 {
        return Err("invalid public transaction size".into());
    }
    let bytes = std::fs::read(path).map_err(|e| error("public transaction", e))?;
    let tx = verify_bytes(&bytes, BranchId::Nu6_3, &mut rand::rng())?;
    if verify_bytes(&bytes, BranchId::Nu7, &mut rand::rng()).is_ok() {
        return Err("old branch accepted as NU7".into());
    }
    Ok(
        serde_json::json!({"scope":"new SDK verification of existing NU6.3 public native transaction; no application-proof verification","private_inputs_read":false,"native_proof_and_signatures_verified":true,"wrong_branch_rejected":true,"txid":tx.txid().to_string(),"bytes":bytes.len(),"chain_inclusion_verified":false,"seconds":started.elapsed().as_secs_f64()}),
    )
}

pub fn gate() -> Result<serde_json::Value> {
    gate_inner(None, None)
}

pub fn gate_export(name: &str) -> Result<serde_json::Value> {
    gate_inner(Some(crate::interop_export::create(name)?), None)
}

pub fn gate_receipts(directory: &std::path::Path) -> Result<serde_json::Value> {
    gate_inner(
        None,
        Some(crate::receipt_transport::Transport::load(directory)?),
    )
}

fn gate_inner(
    export: Option<std::path::PathBuf>,
    receipts: Option<crate::receipt_transport::Transport>,
) -> Result<serde_json::Value> {
    let start = Instant::now();
    let mut rng = rand::rng();
    let mut old_rng = OldOsRng;
    let sk = random_key(&mut rng);
    let fvk = FullViewingKey::from(&sk);
    let keys = split_key(&sk, &mut old_rng)?;
    let amounts = receipts.as_ref().map_or_else(
        || vec![390_000_u64, 600_000],
        |transport| transport.amounts.to_vec(),
    );
    let fee = 5000 * amounts.len() as u64;
    let note = synthetic_note(&fvk, amounts.iter().sum::<u64>() + fee, &mut rng);
    // One fabricated leaf and protocol empty subtrees: never broadcast this gate.
    let path = MerklePath::from_parts(
        0,
        std::array::from_fn(|level| MerkleHashOrchard::empty_root((level as u8).into())),
    );
    let anchor = path.root(note.commitment().into());
    let height = TEST_NETWORK
        .activation_height(NetworkUpgrade::Nu7)
        .ok_or("missing NU7")?
        + 1;
    if BranchId::for_height(&TEST_NETWORK, height) != BranchId::Nu7 {
        return Err("wrong testnet NU7 rules".into());
    }
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
    );
    // Explicit future allowance for the long application proving step on NU7's
    // faster chain. This experiment measures native compatibility, not freshness.
    builder = builder.with_expiry_height(height + 240);
    builder
        .add_ironwood_spend::<Infallible>(fvk.clone(), note, path)
        .map_err(|e| error("synthetic spend", e))?;
    let receivers: Vec<_> = (0..amounts.len())
        .map(|_| FullViewingKey::from(&random_key(&mut rng)))
        .collect();
    for (i, (receiver, amount)) in receivers.iter().zip(&amounts).enumerate() {
        builder
            .add_ironwood_output::<Infallible>(
                None,
                receiver.address_at(0u32, Scope::External),
                Zatoshis::from_u64(*amount).unwrap(),
                receipts
                    .as_ref()
                    .map_or_else(MemoBytes::empty, |v| v.memos[i].clone()),
            )
            .map_err(|e| error("output", e))?;
    }
    let parts = builder
        .build_for_pczt(&mut rng, &FeeRule::standard())
        .map_err(|e| error("PCZT builder", e))?;
    let indices: Vec<usize> = (0..amounts.len())
        .map(|i| parts.ironwood_meta.output_action_index(i).unwrap())
        .collect();
    let pczt = Creator::build_from_parts(parts.pczt_parts).ok_or("PCZT version")?;
    let pczt = IoFinalizer::new(pczt)
        .finalize_io(&mut rng)
        .map_err(|e| error("IO finalizer", e))?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|e| error("effects", e))?;
    if effects.version() != TxVersion::V6
        || effects.consensus_branch_id() != BranchId::Nu7
        || pczt.ironwood().actions().len() != amounts.len()
    {
        return Err("wrong version/branch/actions".into());
    }
    let message: [u8; 32] = v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
    .try_into()
    .map_err(|_| "sighash size")?;
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
        .map_err(|e| error("private-field checks", e))?
        .finish();
    eprintln!("NU7 synthetic native proof started; fabricated funding, never broadcast");
    let proof_started = Instant::now();
    let pk = ProvingKey::build(OrchardCircuitVersion::PostNu6_3);
    let pczt = Prover::new(pczt)
        .create_ironwood_proof(&mut rng, &pk)
        .map_err(|e| error("native proof", e))?
        .finish();
    let proof_seconds = proof_started.elapsed().as_secs_f64();
    let mut real_spends = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<SignError, _>(|_, bundle, _| {
            for action in bundle.actions_mut() {
                if action.spend().spend_auth_sig().is_none() {
                    let alpha = action
                        .spend()
                        .alpha()
                        .as_ref()
                        .ok_or_else(|| SignError::Custom("alpha missing".into()))?;
                    let signature = threshold_signature(&keys, alpha, &message, &mut old_rng)
                        .map_err(SignError::Custom)?;
                    let mut changed = message;
                    changed[0] ^= 1;
                    if action.spend().rk().verify(&changed, &signature).is_ok() {
                        return Err(SignError::Custom("changed digest accepted".into()));
                    }
                    action
                        .apply_signature(message, signature)
                        .map_err(SignError::Apply)?;
                    real_spends += 1;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .finish();
    if real_spends != 1 {
        return Err("wrong threshold spend count".into());
    }
    // Recent upstream low-level signing restores wire viewing keys. Confirm the
    // exact FVKs survive; this is required by full PCZT recipient/private checks.
    let pczt = Verifier::new(pczt)
        .with_ironwood::<String, _>(|bundle| -> std::result::Result<(), OrchardError<String>> {
            let fvks: Vec<_> = bundle
                .actions()
                .iter()
                .map(|a| a.spend().fvk().as_ref().map(FullViewingKey::to_bytes))
                .collect();
            if fvks != before_fvks {
                return Err(OrchardError::Custom("signer changed FVK bytes".into()));
            }
            Ok(())
        })
        .map_err(|e| error("post-sign FVK preservation", e))?
        .finish();
    let tx = TransactionExtractor::new(pczt)
        .extract(&mut rng)
        .map_err(|e| error("extract/verify", e))?;
    if tx.txid().as_ref() != &message {
        return Err("native-only v6 txid/sighash mismatch".into());
    }
    let bundle = tx.ironwood_bundle().ok_or("Ironwood missing")?;
    let mut openings = Vec::new();
    for i in 0..amounts.len() {
        let (note, address, memo) = bundle
            .decrypt_output_with_key(indices[i], &receivers[i].to_ivk(Scope::External))
            .ok_or("recipient decryption")?;
        if note.value().inner() != amounts[i]
            || note.version() != NoteVersion::V3
            || address != receivers[i].address_at(0u32, Scope::External)
            || ExtractedNoteCommitment::from(note.commitment())
                != *bundle.actions()[indices[i]].cmx()
        {
            return Err("recipient note mismatch".into());
        }
        if let Some(transport) = &receipts {
            if memo != *transport.memos[i].as_array() {
                return Err("recipient memo changed during native construction".into());
            }
            for (j, other) in receivers.iter().enumerate() {
                if i != j
                    && bundle
                        .decrypt_output_with_key(indices[i], &other.to_ivk(Scope::External))
                        .is_some()
                {
                    return Err("another recipient decrypted the private memo".into());
                }
            }
        }
        openings.push(serde_json::json!({"action_index":indices[i],"recipient":address.to_raw_address_bytes().to_vec(),
            "amount":note.value().inner(),"rho":note.rho().to_bytes(),"rseed":note.rseed().as_bytes(),
            "cmx":bundle.actions()[indices[i]].cmx().to_bytes()}));
    }
    let mut bytes = Vec::new();
    tx.write(&mut bytes).map_err(|e| error("serialize", e))?;
    verify_bytes(&bytes, BranchId::Nu7, &mut rng)?;
    if verify_bytes(&bytes, BranchId::Nu6_3, &mut rng).is_ok() {
        return Err("NU7 accepted as old branch".into());
    }
    let mut changed = bytes.clone();
    changed.push(0);
    if verify_bytes(&changed, BranchId::Nu7, &mut rng).is_ok() {
        return Err("trailing byte accepted".into());
    }
    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    if verify_bytes(&changed, BranchId::Nu7, &mut rng).is_ok() {
        return Err("binding signature change accepted".into());
    }
    if let Some(transport) = &receipts {
        // Mutate the encrypted memo, not a commitment or signature. Native
        // authorization must reject it, and note AEAD must fail independently.
        let ciphertext = bundle.actions()[indices[0]]
            .encrypted_note()
            .enc_ciphertext
            .as_ref();
        let positions: Vec<_> = bytes
            .windows(ciphertext.len())
            .enumerate()
            .filter_map(|(i, v)| (v == ciphertext).then_some(i))
            .collect();
        if positions.len() != 1 {
            return Err("encrypted memo serialization mapping".into());
        }
        let mut changed = bytes.clone();
        changed[positions[0] + 100] ^= 1;
        if verify_bytes(&changed, BranchId::Nu7, &mut rng).is_ok() {
            return Err("encrypted memo substitution accepted".into());
        }
        let altered = Transaction::read(&changed[..], BranchId::Nu7)
            .map_err(|_| "encrypted memo mutation did not preserve transaction encoding")?;
        if altered
            .ironwood_bundle()
            .ok_or("mutated bundle")?
            .decrypt_output_with_key(indices[0], &receivers[0].to_ivk(Scope::External))
            .is_some()
        {
            return Err("encrypted memo mutation passed AEAD".into());
        }
        transport.persist(&bytes, &tx.txid().to_string(), &receivers, &indices)?;
        return Ok(
            serde_json::json!({"scope":"synthetic NU7 encrypted receipt transport component; no auction output binding or chain funding",
            "branch":"77190ad9","actions":4,"synthetic_inputs":1,"recipient_memos_decrypted":4,
            "cross_recipient_decryptions_rejected":12,"encrypted_memo_mutation_native_rejected":true,
            "encrypted_memo_mutation_aead_rejected":true,"threshold":"2-of-3","setup":"local trusted dealer; no DKG",
            "native_proof_and_signatures_verified":true,"one_share_rejected":true,"changed_sighash_rejected":true,
            "binding_signature_change_rejected":true,"trailing_byte_rejected":true,"pczt_fvks_preserved":true,
            "txid":tx.txid().to_string(),"fee":fee,"transaction_bytes":bytes.len(),
            "proof_seconds":proof_seconds,"total_seconds":start.elapsed().as_secs_f64(),
            "auction_application_binding_verified":false,"chain_funding":false,"broadcast":false}),
        );
    }
    if let Some(directory) = &export {
        let identity = serde_json::json!({"format":1,"scope":"synthetic NU7 output-byte interoperability",
            "sdk":crate::interop_export::SDK,"branch":"77190ad9","txid":tx.txid().to_string()});
        let mut private = identity.clone();
        private["outputs"] = openings.clone().into();
        let mut public = identity;
        public["native_actions"] = bundle
            .actions()
            .iter()
            .map(|a| a.cmx().to_bytes())
            .collect::<Vec<_>>()
            .into();
        public["outputs"] = openings
            .iter()
            .map(|v| serde_json::json!({"action_index":v["action_index"],"cmx":v["cmx"]}))
            .collect::<Vec<_>>()
            .into();
        crate::interop_export::write(&directory.join("synthetic-nu7-transaction.bin"), &bytes)?;
        crate::interop_export::write(
            &directory.join("private-native-outputs.json"),
            &serde_json::to_vec(&private).map_err(|_| "private output encoding")?,
        )?;
        crate::interop_export::write(
            &directory.join("public-output-map.json"),
            &serde_json::to_vec(&public).map_err(|_| "public map encoding")?,
        )?;
        crate::interop_export::verify(directory, None)?;
    }
    Ok(
        serde_json::json!({"scope":"isolated NU7 synthetic native PCZT/FROST compatibility; no auction binding or chain funding","chain_funding":false,"broadcast":false,"branch":"77190ad9","height":u32::from(height),"expiry_height":u32::from(tx.expiry_height()),"version":6,"actions":2,"real_threshold_spends":real_spends,"recipients_decrypted":2,"threshold":"2-of-3","setup":"local trusted dealer; no DKG","native_proof_and_signatures_verified":true,"one_share_rejected":true,"changed_sighash_rejected":true,"old_branch_rejected":true,"trailing_byte_rejected":true,"binding_signature_change_rejected":true,"pczt_fvks_preserved":true,"transaction_bytes":bytes.len(),"fee":10_000,"proof_seconds":proof_seconds,"private_byte_export":export.is_some(),"total_seconds":start.elapsed().as_secs_f64()}),
    )
}

#[derive(Debug)]
pub(crate) enum SignError {
    Parse(OrchardParseError),
    Apply(orchard::pczt::SignerError),
    Custom(String),
}
impl From<OrchardParseError> for SignError {
    fn from(e: OrchardParseError) -> Self {
        Self::Parse(e)
    }
}
impl std::fmt::Display for SignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse: {e:?}"),
            Self::Apply(e) => write!(f, "apply: {e:?}"),
            Self::Custom(e) => write!(f, "{e}"),
        }
    }
}
