//! Default modes are offline synthetic Ironwood/PCZT/FROST gates. Never broadcast
//! their fabricated inputs. The separate --regtest-spend mode consumes captured
//! real isolated-regtest notes; its driver guards the local chain before broadcast.
//! This plaintext oracle fixture is not an MPC-to-settlement binding proof.
mod custody;
mod integration;
mod nu7_auction;
mod nu7_binding;
mod regtest_deposits;
mod regtest_settlement;
mod regtest_spend;
mod row_worker;
use darkcall_auction_core::{Bid, Terms, clear};
use darkcall_settlement_types::{Role, Row};
use ff::{Field, PrimeField};
use group::GroupEncoding;
use incrementalmerkletree::Hashable;
use integration::Integration;
use orchard::{
    Note,
    builder::BundleType,
    circuit::{OrchardCircuitVersion, ProvingKey},
    keys::{FullViewingKey, Scope, SpendAuthorizingKey, SpendValidatingKey, SpendingKey},
    note::{ExtractedNoteCommitment, NoteVersion, RandomSeed, Rho},
    primitives::redpallas::{Signature, SpendAuth},
    tree::{MerkleHashOrchard, MerklePath},
    value::NoteValue,
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
use rand::{RngCore, rngs::OsRng};
use reddsa::frost::redpallas as frost;
use std::{collections::BTreeMap, convert::Infallible, time::Instant};
use zcash_primitives::transaction::{
    TxVersion,
    builder::{BuildConfig, Builder},
    fees::zip317::FeeRule,
    sighash::SignableInput,
    sighash_v6::v6_signature_hash,
    txid::TxIdDigester,
};
use zcash_protocol::{
    consensus::{NetworkUpgrade, Parameters, TEST_NETWORK},
    memo::MemoBytes,
    value::Zatoshis,
};

type Result<T> = std::result::Result<T, String>;
fn error(context: &str, e: impl std::fmt::Debug) -> String {
    format!("{context}: {e:?}")
}

fn random_key(rng: &mut OsRng) -> SpendingKey {
    loop {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        if let Some(key) = SpendingKey::from_bytes(bytes).into_option() {
            return key;
        }
    }
}

fn synthetic_note(fvk: &FullViewingKey, value: u64, rng: &mut OsRng) -> Note {
    let recipient = fvk.address_at(0u32, Scope::External);
    loop {
        let rho = Rho::from_bytes(&pallas::Base::random(&mut *rng).to_repr()).unwrap();
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        if let Some(seed) = RandomSeed::from_bytes(bytes, &rho).into_option()
            && let Some(note) = Note::from_parts(
                recipient,
                NoteValue::from_raw(value),
                rho,
                seed,
                NoteVersion::V3,
            )
            .into_option()
        {
            return note;
        }
    }
}

// Four real leaves followed by the protocol's empty subtrees.
fn paths(notes: &[Note; 4]) -> [MerklePath; 4] {
    let leaves = notes
        .each_ref()
        .map(|n| MerkleHashOrchard::from_cmx(&n.commitment().into()));
    let pairs: [MerkleHashOrchard; 2] = std::array::from_fn(|i| {
        MerkleHashOrchard::combine(0.into(), &leaves[i * 2], &leaves[i * 2 + 1])
    });
    std::array::from_fn(|i| {
        MerklePath::from_parts(
            i as u32,
            std::array::from_fn(|level| match level {
                0 => leaves[i ^ 1],
                1 => pairs[(i / 2) ^ 1],
                _ => MerkleHashOrchard::empty_root((level as u8).into()),
            }),
        )
    })
}

struct ThresholdKeys {
    packages: BTreeMap<frost::Identifier, frost::keys::KeyPackage>,
    public: frost::keys::PublicKeyPackage,
}

fn split_key(sk: &SpendingKey, rng: &mut OsRng) -> Result<ThresholdKeys> {
    // Local test trusted-dealer setup only. Deployment must use a reviewed setup.
    let ask = SpendAuthorizingKey::from(sk);
    let bytes: [u8; 32] = ask.randomize(&pallas::Scalar::zero()).into();
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

fn threshold_signature(
    keys: &ThresholdKeys,
    alpha: &pallas::Scalar,
    message: &[u8; 32],
    rng: &mut OsRng,
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

#[derive(Debug)]
enum SignError {
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

fn gate(
    prove: bool,
    zero_rows: bool,
    integration: Option<&Integration>,
) -> Result<serde_json::Value> {
    let start = Instant::now();
    let mut rng = OsRng;
    let terms = Terms {
        units: 3,
        reserve: if zero_rows { 100 } else { 20 },
        budget: 100,
        processing_fee: if zero_rows { 7500 } else { 15000 },
    };
    let ids = integration
        .map(|i| i.context.ticket_ids)
        .unwrap_or_else(|| [90_u64, 70, 60, 50].map(|v| [v as u8; 32]));
    // Plaintext oracle is only the standalone regression fixture. Integrated
    // signing consumes proved private fill delivery; no max bids are read here.
    let settlement = if integration.is_none() {
        let bids = std::array::from_fn::<_, 4, _>(|i| Bid {
            ticket_id: ids[i],
            amount: [90, 70, 60, 50][i],
        });
        Some(clear(terms, &bids).map_err(|e| error("oracle", e))?)
    } else {
        None
    };
    let network_fee = integration
        .map(|i| i.delivery.fee)
        .unwrap_or_else(|| settlement.as_ref().unwrap().network_fee);
    let escrow_key = random_key(&mut rng);
    let fvk = FullViewingKey::from(&escrow_key);
    let keys = split_key(&escrow_key, &mut rng)?;
    let notes: [Note; 4] = std::array::from_fn(|_| {
        synthetic_note(&fvk, terms.budget + terms.processing_fee, &mut rng)
    });
    let paths = paths(&notes);
    let anchor = paths[0].root(notes[0].commitment().into());
    for (note, path) in notes.iter().zip(&paths) {
        if path.root(note.commitment().into()) != anchor {
            return Err("synthetic tree roots disagree".into());
        }
    }
    let height = TEST_NETWORK
        .activation_height(NetworkUpgrade::Nu6_3)
        .ok_or("missing NU6.3 testnet height")?
        + 1;
    let mut builder = Builder::new(
        TEST_NETWORK,
        height,
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
            ironwood_anchor: Some(anchor),
            orchard_pool_bundle_type: BundleType::DEFAULT,
        },
    );
    for (note, path) in notes.into_iter().zip(paths) {
        builder
            .add_ironwood_spend::<Infallible>(fvk.clone(), note, path)
            .map_err(|e| error("spend", e))?;
    }
    // Canonical book row order, rather than the oracle's sorted-ticket order.
    let amounts: Vec<u64> = if let Some(integration) = integration {
        integration.delivery.amounts.to_vec()
    } else {
        let settlement = settlement.as_ref().unwrap();
        ids.iter()
            .map(|id| {
                settlement
                    .refunds
                    .iter()
                    .find(|row| row.ticket_id == *id)
                    .unwrap()
                    .refunded
            })
            .chain([settlement.seller_amount, settlement.operator_amount])
            .collect()
    };
    let receivers: Vec<_> = amounts
        .iter()
        .map(|_| FullViewingKey::from(&random_key(&mut rng)))
        .collect();
    for (receiver, amount) in receivers.iter().zip(&amounts) {
        builder
            .add_ironwood_output::<Infallible>(
                None,
                receiver.address_at(0u32, Scope::External),
                Zatoshis::from_u64(*amount).map_err(|e| error("amount", e))?,
                MemoBytes::empty(),
            )
            .map_err(|e| error("output", e))?;
    }
    // Use standard ZIP 317 wallet policy, and check against the oracle's shape.
    let parts = builder
        .build_for_pczt(rng, &FeeRule::standard())
        .map_err(|e| error("build PCZT", e))?;
    let output_indices: Vec<usize> = (0..amounts.len())
        .map(|i| {
            parts
                .ironwood_meta
                .output_action_index(i)
                .ok_or("output index missing")
        })
        .collect::<std::result::Result<_, _>>()?;
    let pczt = Creator::build_from_parts(parts.pczt_parts).ok_or("unsupported PCZT version")?;
    if *pczt.global().tx_version() != 6
        || pczt.ironwood().actions().len() != 6
        || !pczt.orchard().actions().is_empty()
    {
        return Err("unexpected transaction version or pool shape".into());
    }
    let pczt = IoFinalizer::new(pczt)
        .finalize_io()
        .map_err(|e| error("finalize IO", e))?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|e| error("effects", e))?;
    if effects.version() != TxVersion::V6 {
        return Err("sighash must use v6".into());
    }
    // Check explicit zero-value slots as well as nonzero outputs, before proving.
    let effects_bundle = effects
        .ironwood_bundle()
        .ok_or("missing effecting bundle")?;
    for (i, (receiver, amount)) in receivers.iter().zip(&amounts).enumerate() {
        let (note, recipient, _) = effects_bundle
            .decrypt_output_with_key(output_indices[i], &receiver.to_ivk(Scope::External))
            .ok_or("constructed output cannot be decrypted")?;
        if note.value().inner() != *amount
            || recipient != receiver.address_at(0u32, Scope::External)
        {
            return Err("constructed receiver or amount mismatch".into());
        }
    }
    let message: [u8; 32] = v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
    .try_into()
    .map_err(|_| "unexpected v6 sighash length")?;
    let construction_seconds = start.elapsed().as_secs_f64();
    let application_started = Instant::now();
    let approval = if let Some(integration) = integration {
        let mut native_notes = Vec::new();
        let mut rows = Vec::new();
        for i in 0..6 {
            let (note, recipient, _) = effects_bundle
                .decrypt_output_with_key(output_indices[i], &receivers[i].to_ivk(Scope::External))
                .ok_or("binding output decryption failed")?;
            let mut salt = [0; 32];
            rng.fill_bytes(&mut salt);
            rows.push(Row {
                conditions: integration.context.conditions,
                accepted_root: integration.context.accepted_root,
                ticket: if i < 4 { ids[i] } else { [0; 32] },
                role: if i < 4 {
                    Role::Refund
                } else if i == 4 {
                    Role::Seller
                } else {
                    Role::Operator
                },
                action_index: output_indices[i] as u32,
                gd: recipient.g_d().to_bytes(),
                pkd: recipient.pk_d().inner().to_bytes(),
                amount: amounts[i],
                salt,
            });
            native_notes.push(note);
        }
        let rows: [Row; 6] = rows.try_into().map_err(|_| "wrong row count")?;
        let native_notes: [Note; 6] = native_notes.try_into().map_err(|_| "wrong note count")?;
        let actions: Vec<_> = effects_bundle
            .actions()
            .iter()
            .map(|a| a.cmx().to_bytes())
            .collect();
        let approved = integration.prove_and_check(
            &rows,
            &native_notes,
            actions.try_into().map_err(|_| "wrong action count")?,
            message,
        )?;
        let original: Vec<_> = effects_bundle
            .actions()
            .iter()
            .map(|a| a.cmx().to_bytes())
            .collect();
        approved.check(&original, &message)?;
        let mut changed = original.clone();
        changed[0][0] ^= 1;
        if approved.check(&changed, &message).is_ok() {
            return Err("substituted native output approved for signing".into());
        }
        let mut changed_digest = message;
        changed_digest[0] ^= 1;
        if approved.check(&original, &changed_digest).is_ok() {
            return Err("substituted effects digest approved for signing".into());
        }
        eprintln!("Application proof and exact native outputs verified before FROST signing");
        Some(approved)
    } else {
        None
    };
    let application_seconds = application_started.elapsed().as_secs_f64();
    // Fully verify private note fields before the low-level signer skips FVK parsing.
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
        .map_err(|e| error("verify private fields", e))?
        .finish();
    let proof_started = Instant::now();
    let pczt = if prove {
        eprintln!("native proving key and six-action proof started");
        let pk = ProvingKey::build(OrchardCircuitVersion::PostNu6_3);
        Prover::new(pczt)
            .create_ironwood_proof(&pk)
            .map_err(|e| error("Ironwood proof", e))?
            .finish()
    } else {
        pczt
    };
    let proof_seconds = proof_started.elapsed().as_secs_f64();
    if let Some(approved) = &approval {
        let current = pczt
            .clone()
            .into_effects()
            .map_err(|e| error("signer effects", e))?;
        let digest: [u8; 32] = v6_signature_hash(
            &current,
            &SignableInput::Shielded,
            &current.digest(TxIdDigester),
        )
        .as_ref()
        .try_into()
        .map_err(|_| "signer digest length")?;
        let actions: Vec<_> = current
            .ironwood_bundle()
            .ok_or("missing signer bundle")?
            .actions()
            .iter()
            .map(|a| a.cmx().to_bytes())
            .collect();
        approved.check(&actions, &digest)?;
    }
    let mut real_spends = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<SignError, _>(|_, bundle, _| {
            for action in bundle.actions_mut() {
                if action.spend().spend_auth_sig().is_none() {
                    let alpha = action
                        .spend()
                        .alpha()
                        .as_ref()
                        .ok_or_else(|| SignError::Custom("missing alpha".into()))?;
                    let signature = threshold_signature(&keys, alpha, &message, &mut rng)
                        .map_err(SignError::Custom)?;
                    let mut substituted = message;
                    substituted[0] ^= 1;
                    if action.spend().rk().verify(&substituted, &signature).is_ok() {
                        return Err(SignError::Custom(
                            "changed transaction hash accepted".into(),
                        ));
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
    if real_spends != 4 {
        return Err("expected four real threshold spends".into());
    }
    let mut summary = serde_json::json!({
        "scope":"offline synthetic native transaction gate", "chain_funding":false,
        "broadcast":false,"mpc_settlement_binding":approval.is_some(),"native_transaction_verified":false,
        "fills_from_plaintext_oracle_fixture":integration.is_none(),"private_fill_delivery_verified":integration.is_some(),
        "private_winner_flags_verified":integration.is_some_and(|i| i.delivery.winners.is_some()),
        "maximum_bids_read":integration.is_none(),"signed_funding_or_receiver_receipts":false,
        "application_proof_checked_before_signing":approval.is_some(),
        "native_output_and_effects_substitution_rejected":approval.is_some(),
        "version":6,"pool":"Ironwood","real_spends":real_spends,"actions":6,
        "requested_outputs":6,"planned_network_fee":network_fee,
        "zero_value_outputs":amounts.iter().filter(|x|**x==0).count(),
        "constructed_recipients_decrypted":amounts.len(),
        "threshold":"2-of-3","setup":"synthetic trusted dealer",
        "one_signature_share_rejected":true,"changed_sighash_rejected":true,
        "construction_seconds":construction_seconds,"application_proof_seconds":application_seconds,"proof_seconds":proof_seconds,
    });
    if prove {
        let tx = TransactionExtractor::new(pczt)
            .extract()
            .map_err(|e| error("extract and verify", e))?;
        if tx.txid().as_ref() != &message {
            return Err("native-only v6 txid differs from shielded signature digest".into());
        }
        let bundle = tx
            .ironwood_bundle()
            .ok_or("missing final Ironwood bundle")?;
        if let Some(approved) = &approval {
            let actions: Vec<_> = bundle
                .actions()
                .iter()
                .map(|a| a.cmx().to_bytes())
                .collect();
            approved.check(&actions, &message)?;
            summary["application_mpc_run"] = approved
                .mpc
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
                .into();
        }
        for (i, (receiver, amount)) in receivers.iter().zip(&amounts).enumerate() {
            let (note, recipient, _) = bundle
                .decrypt_output_with_key(output_indices[i], &receiver.to_ivk(Scope::External))
                .ok_or("recipient cannot decrypt note")?;
            if note.value().inner() != *amount
                || recipient != receiver.address_at(0u32, Scope::External)
                || note.version() != NoteVersion::V3
            {
                return Err("recipient note does not match requested output".into());
            }
            let actual: ExtractedNoteCommitment = note.commitment().into();
            if &actual != bundle.actions()[output_indices[i]].cmx() {
                return Err("decrypted commitment differs from transaction".into());
            }
        }
        let mut bytes = Vec::new();
        tx.write(&mut bytes)
            .map_err(|e| error("transaction bytes", e))?;
        if let Some(integration) = integration {
            // The fabricated transaction is useful for public verifier replay;
            // it must never be submitted to any node.
            std::fs::write(
                integration.directory.join("synthetic-transaction.bin"),
                &bytes,
            )
            .map_err(|e| error("write synthetic transaction", e))?;
        }
        summary["native_transaction_verified"] = true.into();
        summary["recipients_decrypted"] = amounts.len().into();
        summary["transaction_bytes"] = bytes.len().into();
        summary["txid"] = tx.txid().to_string().into();
        summary["native_proof_bytes"] = bundle.authorization().proof().as_ref().len().into();
    }
    summary["total_seconds"] = start.elapsed().as_secs_f64().into();
    Ok(summary)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--nu7-warm-auction-worker" {
        return row_worker::run_auctions(args[1].parse().map_err(|_| "worker concurrency")?);
    }
    if args.len() == 4 && args[0] == "--nu7-funded-memo-resume" {
        println!(
            "{}",
            serde_json::to_string_pretty(&nu7_auction::resume_funded(
                std::path::Path::new(&args[1]),
                std::path::Path::new(&args[2]),
                &args[3]
            )?)
            .map_err(|_| "funded resume summary encoding")?
        );
        return Ok(());
    }
    if args.len() == 7
        && matches!(
            args[0].as_str(),
            "--nu7-funded-memo-prepare" | "--nu7-funded-memo-full"
        )
    {
        println!(
            "{}",
            serde_json::to_string_pretty(&nu7_auction::prepare_funded(
                std::path::Path::new(&args[1]),
                std::path::Path::new(&args[2]),
                std::path::Path::new(&args[3]),
                std::path::Path::new(&args[4]),
                std::path::Path::new(&args[5]),
                &args[6],
                args[0] == "--nu7-funded-memo-full"
            )?)
            .map_err(|_| "funded preparation summary encoding")?
        );
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--prepare-params" {
        // Compute (checked against the pinned digest) and cache the fixed Halo2
        // parameters, e.g. while building an image, so the first proof or
        // verification does not spend about 90 s on them.
        let started = std::time::Instant::now();
        let (_, cached) =
            darkcall_output_binding::poseidon_rows::fixed_params().map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::json!({"params_already_cached": cached,
            "seconds": started.elapsed().as_secs_f64()})
        );
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--row-capacity" {
        // Preflight for DARKCALL_ROW_WORKERS, so a host that fails the memory
        // guard is refused before deposits rather than at proving time.
        let workers = row_worker::funded_row_workers()?;
        println!("{}", serde_json::json!({"row_workers": workers}));
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--warm-row-worker" {
        return row_worker::run(args[1].parse().map_err(|_| "worker concurrency")?);
    }
    if args.len() == 3
        && matches!(
            args[0].as_str(),
            "--nu7-auction-memo-prepare-only" | "--nu7-integrated-memos-shared"
        )
    {
        println!(
            "{}",
            serde_json::to_string_pretty(&nu7_auction::run_with_memos(
                std::path::Path::new(&args[1]),
                &args[2],
                args[0] == "--nu7-integrated-memos-shared",
                true
            )?)
            .map_err(|_| "memo auction summary")?
        );
        return Ok(());
    }
    if args.len() == 3
        && matches!(
            args[0].as_str(),
            "--nu7-integrated-shared" | "--nu7-auction-prepare-only"
        )
    {
        println!(
            "{}",
            nu7_auction::run(
                std::path::Path::new(&args[1]),
                &args[2],
                args[0] == "--nu7-integrated-shared"
            )?
        );
        return Ok(());
    }
    if args.len() == 3 && args[0] == "--verify-nu7-funded-application" {
        println!(
            "{}",
            serde_json::to_string_pretty(&nu7_auction::verify_funded_application(
                std::path::Path::new(&args[1]),
                std::path::Path::new(&args[2])
            )?)
            .map_err(|_| "funded verification summary")?
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--verify-nu7-auction-application" {
        println!(
            "{}",
            nu7_auction::verify_application(std::path::Path::new(&args[1]))?
        );
        return Ok(());
    }
    if args.len() == 3 && args[0] == "--integration-delivery-check" {
        let checked = Integration::new(std::path::Path::new(&args[1]), &args[2])?;
        println!("{}", checked.delivery_gate_summary());
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--native-binding-circuit-profile" {
        println!(
            "{}",
            darkcall_output_binding::poseidon_rows::circuit_profile()
                .map_err(|e| error("circuit profile", e))?
        );
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--shared-binding-circuit-profile" {
        println!(
            "{}",
            darkcall_output_binding::poseidon_rows::shared_circuit_profile()
                .map_err(|e| error("shared circuit profile", e))?
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--nu7-shared-binding-mock" {
        println!(
            "{}",
            nu7_binding::shared_mock(std::path::Path::new(&args[1]))?
        );
        return Ok(());
    }
    if args.len() == 3 && args[0] == "--nu7-shared-output-binding" {
        println!(
            "{}",
            nu7_binding::prove_shared(std::path::Path::new(&args[1]), &args[2])?
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--verify-nu7-shared-output-binding" {
        println!(
            "{}",
            nu7_binding::verify_shared(std::path::Path::new(&args[1]))?
        );
        return Ok(());
    }
    if args.len() == 3 && args[0] == "--nu7-output-binding" {
        println!(
            "{}",
            serde_json::to_string_pretty(&nu7_binding::prove(
                std::path::Path::new(&args[1]),
                &args[2]
            )?)
            .map_err(|_| "binding summary")?
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--verify-nu7-output-binding" {
        println!(
            "{}",
            serde_json::to_string_pretty(&nu7_binding::verify(std::path::Path::new(&args[1]))?)
                .map_err(|_| "binding replay summary")?
        );
        return Ok(());
    }
    if args.len() == 8 && args[0] == "--regtest-resume-built" {
        println!(
            "{}",
            serde_json::to_string_pretty(&regtest_settlement::resume_built(
                std::path::Path::new(&args[1]),
                std::path::Path::new(&args[2]),
                std::path::Path::new(&args[3]),
                std::path::Path::new(&args[4]),
                std::path::Path::new(&args[5]),
                &args[6],
                std::path::Path::new(&args[7]),
            )?)
            .map_err(|e| error("built resume summary", e))?
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--regtest-dkg-init" {
        println!(
            "{}",
            serde_json::to_string_pretty(&custody::init(&args[1])?)
                .map_err(|e| error("DKG summary", e))?
        );
        return Ok(());
    }
    if args.len() == 7
        && matches!(
            args[0].as_str(),
            "--regtest-settlement" | "--regtest-settlement-shared"
        )
    {
        let build = if args[0] == "--regtest-settlement-shared" {
            regtest_settlement::build_shared
        } else {
            regtest_settlement::build
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&build(
                std::path::Path::new(&args[1]),
                std::path::Path::new(&args[2]),
                std::path::Path::new(&args[3]),
                std::path::Path::new(&args[4]),
                std::path::Path::new(&args[5]),
                &args[6],
            )?)
            .map_err(|e| error("funded settlement summary", e))?
        );
        return Ok(());
    }
    if args.len() == 7 && args[0] == "--regtest-deposits" {
        println!(
            "{}",
            serde_json::to_string_pretty(&regtest_deposits::build(
                std::path::Path::new(&args[1]),
                std::path::Path::new(&args[2]),
                std::path::Path::new(&args[3]),
                std::path::Path::new(&args[4]),
                std::path::Path::new(&args[5]),
                &args[6],
            )?)
            .map_err(|e| error("deposit summary", e))?
        );
        return Ok(());
    }
    if args.len() == 5 && args[0] == "--regtest-spend" {
        println!(
            "{}",
            serde_json::to_string_pretty(&regtest_spend::build(
                std::path::Path::new(&args[1]),
                std::path::Path::new(&args[2]),
                std::path::Path::new(&args[3]),
                &args[4]
            )?)
            .map_err(|e| error("regtest summary", e))?
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--verify-integrated" {
        println!(
            "{}",
            serde_json::to_string_pretty(&integration::verify_public(
                std::path::Path::new(&args[1]),
                None
            )?)
            .map_err(|e| error("public summary", e))?
        );
        return Ok(());
    }
    if args.len() == 3 && args[0] == "--verify-certified-integrated" {
        println!(
            "{}",
            serde_json::to_string_pretty(&integration::verify_public(
                std::path::Path::new(&args[1]),
                Some(std::path::Path::new(&args[2]))
            )?)
            .map_err(|e| error("certified public summary", e))?
        );
        return Ok(());
    }
    let integrated = if args.len() == 3
        && matches!(args[0].as_str(), "--integrated" | "--integrated-shared")
    {
        let layout = if args[0] == "--integrated-shared" {
            darkcall_output_binding::poseidon_rows::RowLayout::Shared18
        } else {
            darkcall_output_binding::poseidon_rows::RowLayout::Working33
        };
        Some(Integration::new_with_layout(
            std::path::Path::new(&args[1]),
            &args[2],
            layout,
        )?)
    } else {
        if args.iter().any(|x| x != "--prove" && x != "--zero-rows") {
            return Err("usage: darkcall-native-gate [--prove] [--zero-rows] | --integrated BOOK_RUN LOCAL_NAME".into());
        }
        None
    };
    let prove = integrated.is_some() || args.iter().any(|x| x == "--prove");
    let zero_rows = args.iter().any(|x| x == "--zero-rows");
    println!(
        "{}",
        serde_json::to_string_pretty(&gate(prove, zero_rows, integrated.as_ref())?)
            .map_err(|e| error("summary", e))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_pczt_threshold_and_recipient_gate() {
        let result = gate(false, false, None).unwrap();
        assert_eq!(result["real_spends"], 4);
        assert_eq!(result["constructed_recipients_decrypted"], 6);
        assert_eq!(result["one_signature_share_rejected"], true);
    }
    #[test]
    fn zero_seller_and_operator_rows_keep_native_shape() {
        let result = gate(false, true, None).unwrap();
        assert_eq!(result["zero_value_outputs"], 2);
        assert_eq!(result["actions"], 6);
        assert_eq!(result["constructed_recipients_decrypted"], 6);
    }
}
