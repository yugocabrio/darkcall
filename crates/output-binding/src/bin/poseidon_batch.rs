//! Six native rows with a reused Poseidon key and sequential proofs. Output-only; no chain
//! funding, custody signatures or full auction connection is claimed here.
use darkcall_bn254_emulation::poseidon::{digest_words, pack_row, reference};
use darkcall_output_binding::poseidon_rows::{
    HASH_RULE, K, RowProver, binding_from_note, row_public, verify_set,
};
use darkcall_settlement_types::{
    NativeBatchStatement, NativeContext, Role, Row, link_four_bid_statements,
};
use ff::Field;
use group::GroupEncoding;
use orchard::{
    builder::{Builder, BundleType},
    bundle::{BundleVersion, Flags},
    keys::{FullViewingKey, Scope, SpendingKey},
    value::NoteValue,
};
use pasta_curves::pallas;
use rand::{RngCore, rngs::OsRng};
use std::time::Instant;

fn verify_export(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    if std::fs::metadata(path.join("native-public.json"))?.len() > 32768 {
        return Err("statement too large".into());
    }
    let statement: NativeBatchStatement =
        serde_json::from_slice(&std::fs::read(path.join("native-public.json"))?)?;
    let mut proofs = Vec::new();
    for i in 0..6 {
        let file = path.join(format!("native-binding-proof-{i}"));
        if std::fs::metadata(&file)?.len() > 65536 {
            return Err("proof too large".into());
        }
        proofs.push(std::fs::read(file)?);
    }
    verify_set(&statement, &proofs)?;
    println!(
        "{}",
        serde_json::json!({"proof_verified":true,"trusted_circuit_key":true,"private_openings_read":false,"rows":6,"hash_rule":HASH_RULE,"seconds":started.elapsed().as_secs_f64(),"chain_inclusion_or_funding_proven":false})
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--verify-export" {
        return verify_export(std::path::Path::new(&args[1]));
    }
    let mut export_name = None;
    let mut context_path = None;
    for pair in args.chunks(2) {
        if pair.len() != 2 {
            return Err("expected option/value pairs".into());
        }
        match pair[0].as_str() {
            "--export-native-metadata" if export_name.is_none() => export_name = Some(&pair[1]),
            "--book-context" if context_path.is_none() => context_path = Some(&pair[1]),
            _ => {
                return Err(
                    "usage: poseidon_batch [--export-native-metadata NAME] [--book-context CONTEXT]"
                        .into(),
                );
            }
        }
    }
    let mut rng = OsRng;
    let amounts = [50u64, 50, 50, 100, 150, 30000];
    let receivers: [FullViewingKey; 6] = std::array::from_fn(|_| {
        let key = loop {
            let mut bytes = [0; 32];
            rng.fill_bytes(&mut bytes);
            if let Some(key) = SpendingKey::from_bytes(bytes).into_option() {
                break key;
            }
        };
        FullViewingKey::from(&key)
    });
    let mut builder = Builder::new(
        BundleType::DEFAULT,
        BundleVersion::ironwood_v3(),
        Flags::SPENDS_DISABLED,
        orchard::Anchor::empty_tree(),
    )
    .unwrap();
    for (receiver, amount) in receivers.iter().zip(amounts) {
        builder
            .add_output(
                None,
                receiver.address_at(0u32, Scope::External),
                NoteValue::from_raw(amount),
                [0; 512],
            )
            .unwrap();
    }
    let (bundle, metadata) = builder.build::<i64>(&mut rng).unwrap().unwrap();
    assert_eq!(bundle.actions().len(), 6);
    let context = if let Some(path) = context_path {
        if std::fs::metadata(path)?.len() > 4096 {
            return Err("context file too large".into());
        }
        serde_json::from_slice::<NativeContext>(&std::fs::read(path)?)?
    } else {
        let mut conditions = [0; 32];
        let mut accepted_root = [0; 32];
        rng.fill_bytes(&mut conditions);
        rng.fill_bytes(&mut accepted_root);
        accepted_root[0] &= 31; // synthetic; not authenticated funding
        NativeContext {
            conditions,
            accepted_root,
            ticket_ids: std::array::from_fn(|i| [(i + 1) as u8; 32]),
        }
    };
    let mut openings = Vec::new();
    let mut private_rows = Vec::new();
    let mut hashes = Vec::new();
    let mut indices = Vec::new();
    let mut cmxs = Vec::new();
    for i in 0..6 {
        let action = metadata.output_action_index(i).unwrap();
        let (note, recipient, _) = bundle
            .decrypt_output_with_key(action, &receivers[i].to_ivk(Scope::External))
            .unwrap();
        assert_eq!(recipient, receivers[i].address_at(0u32, Scope::External));
        assert_eq!(note.value().inner(), amounts[i]);
        let gd = recipient.g_d();
        let pkd = recipient.pk_d().inner();
        let mut salt = [0u8; 32];
        rng.fill_bytes(&mut salt);
        let row = Row {
            conditions: context.conditions,
            accepted_root: context.accepted_root,
            ticket: if i < 4 {
                context.ticket_ids[i]
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
            action_index: action as u32,
            gd: gd.to_bytes(),
            pkd: pkd.to_bytes(),
            amount: amounts[i],
            salt,
        };
        openings.push(binding_from_note(&row, &note)?);
        let row = row.encode();
        let words = digest_words(&reference(&pack_row(&row)));
        let cmx = bundle.actions()[action].cmx().to_bytes();
        private_rows.push(row.to_vec());
        hashes.push(words);
        indices.push(action);
        cmxs.push(cmx);
    }
    let statement = NativeBatchStatement {
        row_hashes: hashes.try_into().map_err(|_| "wrong row count")?,
        action_indices: indices
            .iter()
            .map(|i| *i as u32)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "wrong index count")?,
        cmxs: cmxs.try_into().map_err(|_| "wrong cmx count")?,
        native_actions: bundle
            .actions()
            .iter()
            .map(|a| a.cmx().to_bytes())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "wrong action count")?,
    };
    link_four_bid_statements(
        &statement.row_hashes,
        &statement.bindings(),
        &statement.native_actions,
    )
    .map_err(|_| "invalid action mapping")?;
    let mut export_directory = None;
    if let Some(name) = export_name {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(
                "usage: poseidon_batch [--export-native-metadata LOCAL_FIXTURE_NAME]".into(),
            );
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(".local")
            .canonicalize()?;
        let directory = root.join(name);
        std::fs::create_dir(&directory)?;
        std::fs::write(
            directory.join("native-private-rows.json"),
            serde_json::to_vec_pretty(&private_rows)?,
        )?;
        std::fs::write(
            directory.join("native-public.json"),
            serde_json::to_vec_pretty(&statement)?,
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                directory.join("native-private-rows.json"),
                std::fs::Permissions::from_mode(0o600),
            )?;
        }
        export_directory = Some(directory);
    }
    eprintln!("Reused native Poseidon key generation started");
    let key_started = Instant::now();
    let prover = RowProver::new()?;
    let key_seconds = key_started.elapsed().as_secs_f64();
    let mut proofs = Vec::new();
    let mut row_seconds = Vec::new();
    for (i, opening) in openings.into_iter().enumerate() {
        eprintln!("Native Poseidon row {i} proof started");
        let public = row_public(&statement, i)?;
        let proof_started = Instant::now();
        let proof = prover.prove(opening, &public)?;
        row_seconds.push(proof_started.elapsed().as_secs_f64());
        for index in [0, 8, 9] {
            let mut changed = public.clone();
            changed[index] += pallas::Base::ONE;
            assert!(prover.verify(&proof, &changed).is_err());
        }
        if let Some(directory) = &export_directory {
            std::fs::write(directory.join(format!("native-binding-proof-{i}")), &proof)?;
        }
        proofs.push(proof);
        eprintln!("Native Poseidon row {i} passed");
    }
    // Row/action transplantation must fail even with otherwise valid proofs.
    assert!(
        prover
            .verify(&proofs[0], &row_public(&statement, 1)?)
            .is_err()
    );
    let summary = serde_json::json!({"scope":"six synthetic native outputs bound to Poseidon rows, reused key and sequential proofs", "hash_rule":HASH_RULE,"k":K,"rows":6,"actions":6,"proof_bytes":proofs.iter().map(|p|p.len()).sum::<usize>(),"individual_proof_bytes":proofs.iter().map(|p|p.len()).collect::<Vec<_>>(),"proof_verified":true,"changed_row_cmx_index_rejected":true,"row_transplant_rejected":true,"auction_plan_connected":false,"book_context_supplied":context_path.is_some(),"funding_proven":false,"native_custody_signature_verified":false,"classical_binding_assumption":true,"rseed_derivation_constrained":false,"ciphertext_constrained":false,"key_seconds":key_seconds,"row_proof_and_verify_seconds":row_seconds,"seconds":started.elapsed().as_secs_f64()});
    if let Some(directory) = export_directory {
        std::fs::write(
            directory.join("native-proof-summary.json"),
            serde_json::to_vec_pretty(&summary)?,
        )?;
    }
    println!("{summary}");
    Ok(())
}
