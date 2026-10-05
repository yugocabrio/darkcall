//! Actual isolated chain funds -> certified private auction -> exact native
//! settlement. Local trusted dealer; no client spending keys are loaded.
use super::*;
use crate::regtest_spend::{
    Regtest, WalletNotes, read_private, reconstruct, wallet_key, write_private,
};
use darkcall_input_tool::{context, receiver_commitment};
use darkcall_wallet_bridge::{
    final_book::{Certificate, Manifest},
    journal::{Event, Journal},
};
use serde::Deserialize;
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receiver {
    receiver: Vec<u8>,
    nonce: [u8; 32],
}
pub fn build(
    escrow: &Path,
    notes: &Path,
    book: &Path,
    receivers: &Path,
    admission: &Path,
    name: &str,
) -> Result<serde_json::Value> {
    let start = Instant::now();
    let key = wallet_key(escrow)?;
    let fvk = FullViewingKey::from(&key);
    let state: WalletNotes = serde_json::from_slice(&read_private(notes, 1_000_000)?)
        .map_err(|e| error("funding snapshot", e))?;
    let manifest: Manifest =
        serde_json::from_slice(&read_private(&admission.join("manifest.json"), 16384)?)
            .map_err(|e| error("authority", e))?;
    let certificate: Certificate = serde_json::from_slice(&read_private(
        &admission.join("final-book-certificate.json"),
        131072,
    )?)
    .map_err(|e| error("certificate", e))?;
    certificate.verify(&manifest)?;
    if hex::encode(manifest.context.network_genesis) != state.genesis
        || state.genesis != "029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327"
        || state.scope != "isolated regtest wallet openings"
        || state.nu6_3_height != 2
        || state.notes.len() != 4
    {
        return Err("funded auction requires exactly four isolated-regtest notes".into());
    }
    if SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| error("clock", e))?
        .as_secs()
        < manifest.closes_at_utc
    {
        return Err("auction cutoff has not occurred".into());
    }
    let recs: [Receiver; 6] = serde_json::from_slice(&read_private(receivers, 16384)?)
        .map_err(|e| error("receiver openings", e))?;
    let mut addresses = Vec::new();
    let accepted = &certificate.endorsements[0].book;
    let c = &manifest.context;
    let ctx = context(
        &c.conditions_hash,
        c.units,
        c.reserve,
        c.budget,
        c.processing_fee,
    );
    for (i, r) in recs.iter().enumerate() {
        let bytes: [u8; 43] = r
            .receiver
            .as_slice()
            .try_into()
            .map_err(|_| "receiver length")?;
        let address = orchard::Address::from_raw_address_bytes(&bytes)
            .into_option()
            .ok_or("invalid receiver")?;
        let ticket = if i < 4 {
            accepted.entries[i].ticket
        } else {
            [0; 32]
        };
        let role = if i < 4 {
            Role::Refund
        } else if i == 4 {
            Role::Seller
        } else {
            Role::Operator
        };
        let actual = receiver_commitment(
            ctx,
            &ticket,
            role as u32,
            &address.g_d().to_bytes(),
            &address.pk_d().inner().to_bytes(),
            &r.nonce,
        )?;
        let expected = if i < 4 {
            accepted.entries[i].refund_receiver_commitment
        } else if i == 4 {
            manifest.seller_receiver_commitment
        } else {
            manifest.operator_receiver_commitment
        };
        if actual != expected {
            return Err("receiver opening differs from certified client intent".into());
        }
        addresses.push(address);
    }
    let mut journals = Vec::new();
    for i in 0..3 {
        journals.push(Journal::open(
            &admission.join(format!("private-node-{i}.jsonl")),
            c,
        )?);
    }
    let required = c
        .budget
        .checked_add(c.processing_fee)
        .ok_or("deposit overflow")?;
    let mut funding = Vec::new();
    let mut used = std::collections::BTreeSet::new();
    for entry in &accepted.entries {
        let reg = journals[0]
            .registration(&entry.ticket)
            .ok_or("missing funded registration")?;
        for j in &journals {
            let other = j
                .registration(&entry.ticket)
                .ok_or("operator omitted funded ticket")?;
            if j.phase() != darkcall_auction_core::funding::Phase::Frozen
                || j.registrations().count() != 4
                || other.note != reg.note
                || other.bid_commitment != entry.bid_commitment
                || other.receiver_commitment != entry.refund_receiver_commitment
            {
                return Err("funding journal differs from certified book or is not frozen".into());
            }
        }
        if !used.insert(reg.note) {
            return Err("duplicate funding note".into());
        }
        let opening = state
            .notes
            .iter()
            .find(|n| n.cmx == reg.note)
            .ok_or("accepted input absent from unspent snapshot")?;
        if opening.amount != required
            || opening.coinbase
            || state.tip_height
                < opening
                    .mined_height
                    .checked_add(1)
                    .ok_or("confirmation overflow")?
        {
            return Err("accepted deposit amount/type/confirmations mismatch".into());
        }
        funding.push(reconstruct(opening, &fvk, &state.anchor)?);
    }
    let mut integration = Integration::new(book, name)?;
    if integration.context.conditions != c.conditions_hash
        || integration.context.accepted_root != accepted.accepted_root
        || integration.context.ticket_ids != accepted.entries.each_ref().map(|e| e.ticket)
    {
        return Err("MPC book differs from certified admission".into());
    }
    integration.funding_is_synthetic = false;
    integration.receiver_openings = Some(
        receivers
            .canonicalize()
            .map_err(|e| error("receiver path", e))?,
    );
    let anchor = funding[0].1.root(funding[0].0.commitment().into());
    let mut builder = Builder::new(
        Regtest,
        zcash_protocol::consensus::BlockHeight::from_u32(
            state.tip_height.checked_add(1).ok_or("height overflow")?,
        ),
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
            ironwood_anchor: Some(anchor),
            orchard_pool_bundle_type: BundleType::DEFAULT,
        },
    );
    for (note, path) in funding {
        builder
            .add_ironwood_spend::<Infallible>(fvk.clone(), note, path)
            .map_err(|e| error("funded auction spend", e))?;
    }
    let amounts = integration.delivery.amounts;
    let fee = 30_000u64;
    if integration.delivery.fee != fee
        || amounts
            .iter()
            .try_fold(fee, |a, b| a.checked_add(*b))
            .ok_or("output total overflow")?
            != required.checked_mul(4).ok_or("funding total overflow")?
    {
        return Err("funded conservation/fee mismatch".into());
    }
    let ovk = fvk.to_ovk(Scope::External);
    for (address, amount) in addresses.iter().zip(amounts) {
        builder
            .add_ironwood_output::<Infallible>(
                Some(ovk.clone()),
                *address,
                Zatoshis::from_u64(amount).map_err(|e| error("settlement amount", e))?,
                MemoBytes::empty(),
            )
            .map_err(|e| error("settlement output", e))?;
    }
    let parts = builder
        .build_for_pczt(OsRng, &FeeRule::standard())
        .map_err(|e| error("funded PCZT", e))?;
    let indices: [usize; 6] =
        std::array::from_fn(|i| parts.ironwood_meta.output_action_index(i).unwrap());
    let pczt =
        IoFinalizer::new(Creator::build_from_parts(parts.pczt_parts).ok_or("funded creator")?)
            .finalize_io()
            .map_err(|e| error("funded IO", e))?;
    let effects = pczt
        .clone()
        .into_effects()
        .map_err(|e| error("funded effects", e))?;
    if effects.version() != TxVersion::V6
        || effects
            .ironwood_bundle()
            .ok_or("funded bundle")?
            .actions()
            .len()
            != 6
    {
        return Err("funded shape/version mismatch".into());
    }
    let bundle = effects.ironwood_bundle().unwrap();
    let mut output_notes = Vec::new();
    let mut rows = Vec::new();
    for i in 0..6 {
        let (note, address, _) = bundle
            .recover_output_with_ovk(indices[i], &ovk)
            .ok_or("outgoing native note recovery failed")?;
        if address != addresses[i]
            || note.value().inner() != amounts[i]
            || note.version() != NoteVersion::V3
        {
            return Err("outgoing native ciphertext differs from certified plan".into());
        }
        let mut salt = [0; 32];
        OsRng.fill_bytes(&mut salt);
        rows.push(Row {
            conditions: c.conditions_hash,
            accepted_root: accepted.accepted_root,
            ticket: if i < 4 {
                accepted.entries[i].ticket
            } else {
                [0; 32]
            },
            role: if i < 4 {
                Role::Refund
            } else if i == 4 {
                Role::Seller
            } else {
                Role::Operator
            },
            action_index: indices[i] as u32,
            gd: address.g_d().to_bytes(),
            pkd: address.pk_d().inner().to_bytes(),
            amount: amounts[i],
            salt,
        });
        output_notes.push(note);
    }
    let message: [u8; 32] = v6_signature_hash(
        &effects,
        &SignableInput::Shielded,
        &effects.digest(TxIdDigester),
    )
    .as_ref()
    .try_into()
    .unwrap();
    let actions: [[u8; 32]; 6] = bundle
        .actions()
        .iter()
        .map(|a| a.cmx().to_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let app_start = Instant::now();
    let approval = integration.prove_and_check(
        &rows.try_into().map_err(|_| "row count")?,
        &output_notes.try_into().map_err(|_| "note count")?,
        actions,
        message,
    )?;
    let encoded =
        fs::read(approval.mpc.join("public-input0")).map_err(|e| error("MPC statement", e))?;
    certificate.verify_statement(&encoded, &manifest)?;
    let mut changed = encoded.clone();
    changed[169 * 32 + 31] ^= 1;
    if certificate.verify_statement(&changed, &manifest).is_ok() {
        return Err("altered MPC refund recipient passed certification".into());
    }
    approval.check(&actions, &message)?;
    let mut changed_actions = actions;
    changed_actions[0][0] ^= 1;
    let mut changed_message = message;
    changed_message[0] ^= 1;
    if approval.check(&changed_actions, &message).is_ok()
        || approval.check(&actions, &changed_message).is_ok()
    {
        return Err("substituted output/effects approved".into());
    }
    let app_seconds = app_start.elapsed().as_secs_f64();
    let pczt = Verifier::new(pczt)
        .with_ironwood::<Infallible, _>(
            |bundle| -> std::result::Result<(), OrchardError<Infallible>> {
                bundle.verify_cross_address_restriction()?;
                for a in bundle.actions() {
                    a.verify_cv_net()?;
                    a.spend().verify_nullifier(Some(&fvk))?;
                    a.spend().verify_rk(Some(&fvk))?;
                    a.output().verify_note_commitment(a.spend())?;
                }
                Ok(())
            },
        )
        .map_err(|e| error("funded private fields", e))?
        .finish();
    for j in &mut journals {
        j.append(Event::Prepare {
            effects: message,
            cancellation: false,
        })?;
    }
    let keys = split_key(&key, &mut OsRng)?;
    let proof_start = Instant::now();
    let pczt = Prover::new(pczt)
        .create_ironwood_proof(&ProvingKey::build(OrchardCircuitVersion::PostNu6_3))
        .map_err(|e| error("funded consensus proof", e))?
        .finish();
    let proof_seconds = proof_start.elapsed().as_secs_f64();
    let current = pczt
        .clone()
        .into_effects()
        .map_err(|e| error("pre-sign effects", e))?;
    let current_message: [u8; 32] = v6_signature_hash(
        &current,
        &SignableInput::Shielded,
        &current.digest(TxIdDigester),
    )
    .as_ref()
    .try_into()
    .unwrap();
    let current_actions: Vec<_> = current
        .ironwood_bundle()
        .ok_or("pre-sign bundle")?
        .actions()
        .iter()
        .map(|a| a.cmx().to_bytes())
        .collect();
    approval.check(&current_actions, &current_message)?;
    certificate.verify_statement(&encoded, &manifest)?;
    let mut spends = 0;
    let pczt = Signer::new(pczt)
        .sign_ironwood_with::<SignError, _>(|_, bundle, _| {
            for a in bundle.actions_mut() {
                if a.spend().spend_auth_sig().is_none() {
                    let alpha = a
                        .spend()
                        .alpha()
                        .as_ref()
                        .ok_or_else(|| SignError::Custom("funded alpha".into()))?;
                    let sig = threshold_signature(&keys, alpha, &message, &mut OsRng)
                        .map_err(SignError::Custom)?;
                    a.apply_signature(message, sig).map_err(SignError::Apply)?;
                    spends += 1;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .finish();
    if spends != 4 {
        return Err("expected four funded threshold spends".into());
    }
    let tx = TransactionExtractor::new(pczt)
        .extract()
        .map_err(|e| error("funded extraction", e))?;
    if tx.txid().as_ref() != &message {
        return Err("funded identity changed".into());
    }
    for i in 0..6 {
        let (n, a, _) = tx
            .ironwood_bundle()
            .unwrap()
            .recover_output_with_ovk(indices[i], &ovk)
            .ok_or("final outgoing recovery")?;
        if n.value().inner() != amounts[i] || a != addresses[i] {
            return Err("final note differs".into());
        }
    }
    let mut bytes = Vec::new();
    tx.write(&mut bytes).map_err(|e| error("funded bytes", e))?;
    write_private(
        &integration.directory.join("regtest-transaction.bin"),
        &bytes,
    )?;
    write_private(
        &integration.directory.join("final-book-certificate.json"),
        &serde_json::to_vec_pretty(&certificate).map_err(|e| error("certificate export", e))?,
    )?;
    write_private(
        &integration.directory.join("private-output-map.json"),
        &serde_json::to_vec(
            &serde_json::json!({"indices":indices,"amounts":amounts,"effects":message}),
        )
        .map_err(|e| error("output map", e))?,
    )?;
    let summary = serde_json::json!({"scope":"actual isolated regtest certified four-input native auction construction","chain_funding":"actual SDK-scanned notes; provenance checked by Python driver","funding_observations_fabricated":false,"broadcast":false,"client_spending_keys_read":false,"maximum_bids_read":false,"fills_from_plaintext_oracle_fixture":false,"private_refund_delivery_verified":true,"signed_root_and_recipients_checked_before_frost":true,"native_bindings_and_cpp_mpc_verified":true,"source_inputs_match_all_three_durable_journals":true,"funding_reservations_fsynced_before_signing":true,"changed_recipient_outputs_effects_rejected":true,"inputs":4,"outputs":6,"threshold":"2-of-3","setup":"local trusted dealer; full escrow key retained","native_proof_and_signatures_verified":true,"transaction_bytes":bytes.len(),"fee":fee,"price":integration.delivery.price,"sold":integration.delivery.sold,"application_proof_seconds":app_seconds,"proof_seconds":proof_seconds,"total_seconds":start.elapsed().as_secs_f64(),"txid":tx.txid().to_string()});
    write_private(
        &integration.directory.join("funded-summary.json"),
        &serde_json::to_vec_pretty(&summary).map_err(|e| error("summary", e))?,
    )?;
    Ok(summary)
}
