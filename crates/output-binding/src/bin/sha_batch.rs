//! Six native rows in one binding proof. Synthetic output-only bundle, no chain
//! funding, custody signatures or full auction connection is claimed here.
use darkcall_output_binding::sha::{Batch, Binding, DOMAIN};
use darkcall_settlement_types::{
    NativeBatchStatement, NativeContext, Role, Row, link_four_bid_statements,
};
use ff::Field;
use group::{Curve, GroupEncoding};
use halo2_proofs::{
    circuit::Value,
    dev::MockProver,
    plonk::{Circuit, SingleVerifier, create_proof, keygen_pk, keygen_vk, verify_proof},
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite, Challenge255},
};
use orchard::{
    builder::{Builder, BundleType},
    bundle::{BundleVersion, Flags},
    keys::{FullViewingKey, Scope, SpendingKey},
    note::ExtractedNoteCommitment,
    value::NoteValue,
};
use pasta_curves::{pallas, vesta};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use std::time::Instant;

const K: u32 = 17;

fn verify_export(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let public_path = path.join("native-public.json");
    let proof_path = path.join("native-binding-proof");
    if std::fs::metadata(&public_path)?.len() > 32768
        || std::fs::metadata(&proof_path)?.len() > 65536
    {
        return Err("native statement/proof file too large".into());
    }
    let statement: NativeBatchStatement = serde_json::from_slice(&std::fs::read(public_path)?)?;
    link_four_bid_statements(
        &statement.row_hashes,
        &statement.bindings(),
        &statement.native_actions,
    )
    .map_err(|_| "invalid native action mapping")?;
    let mut public = Vec::new();
    for i in 0..6 {
        public.extend(statement.row_hashes[i].map(|x| pallas::Base::from(x as u64)));
        let cmx = ExtractedNoteCommitment::from_bytes(&statement.cmxs[i])
            .into_option()
            .ok_or("noncanonical native commitment")?;
        public.push(cmx.inner());
        public.push(pallas::Base::from(statement.action_indices[i] as u64));
    }
    let empty = Batch::<6>(std::array::from_fn(|_| Binding::default()));
    let params: Params<vesta::Affine> = Params::new(K);
    let vk = keygen_vk(&params, &empty)?;
    let proof = std::fs::read(proof_path)?;
    let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(&proof[..]);
    verify_proof(
        &params,
        &vk,
        SingleVerifier::new(&params),
        &[&[&public]],
        &mut transcript,
    )?;
    println!(
        "{}",
        serde_json::json!({"proof_verified":true,"trusted_circuit_key":true,
        "private_openings_read":false,"rows":6,"seconds":started.elapsed().as_secs_f64(),
        "chain_inclusion_or_funding_proven":false})
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
                    "usage: sha_batch [--export-native-metadata NAME] [--book-context CONTEXT]"
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
    let mut public = Vec::new();
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
        let rho = note.rho();
        let psi = note.rseed().psi(&rho);
        let rcm = note
            .rseed()
            .rcm_v3(&rho, &gd, &pkd, amounts[i], &psi)
            .inner();
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
        }
        .encode();
        assert_eq!(&row[..16], DOMAIN);
        openings.push(Binding {
            gd: Value::known(gd.to_affine()),
            pkd: Value::known(pkd.to_affine()),
            value: Value::known(amounts[i]),
            rho: Value::known(rho.into_inner()),
            psi: Value::known(psi),
            rcm: Value::known(rcm),
            prefix: std::array::from_fn(|j| {
                Value::known(u32::from_be_bytes(
                    row[16 + j * 4..20 + j * 4].try_into().unwrap(),
                ))
            }),
            salt: std::array::from_fn(|j| {
                Value::known(u32::from_be_bytes(
                    row[192 + j * 4..196 + j * 4].try_into().unwrap(),
                ))
            }),
        });
        let digest = Sha256::digest(row);
        let words: [u32; 8] = std::array::from_fn(|j| {
            u32::from_be_bytes(digest[j * 4..j * 4 + 4].try_into().unwrap())
        });
        public.extend(words.map(|x| pallas::Base::from(x as u64)));
        let cmx = bundle.actions()[action].cmx().to_bytes();
        public.push(ExtractedNoteCommitment::from_bytes(&cmx).unwrap().inner());
        public.push(pallas::Base::from(action as u64));
        private_rows.push(row.to_vec());
        hashes.push(words);
        indices.push(action);
        cmxs.push(cmx);
    }
    let circuit = Batch::<6>(openings.try_into().map_err(|_| "wrong row count")?);
    let mut export_directory = None;
    if let Some(name) = export_name {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("usage: sha_batch [--export-native-metadata LOCAL_FIXTURE_NAME]".into());
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
            serde_json::to_vec_pretty(
                &serde_json::json!({"row_hashes":hashes,"action_indices":indices,"cmxs":cmxs,
                    "native_actions": bundle.actions().iter().map(|a| a.cmx().to_bytes()).collect::<Vec<_>>()}),
            )?,
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
    eprintln!("Six-row native mock started");
    MockProver::run(K, &circuit, vec![public.clone()])?.assert_satisfied();
    eprintln!("Six-row native mock passed; key generation started");
    let params: Params<vesta::Affine> = Params::new(K);
    let vk = keygen_vk(&params, &circuit.without_witnesses())?;
    let pk = keygen_pk(&params, vk, &circuit.without_witnesses())?;
    eprintln!("Six-row native proof started");
    let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(Vec::new());
    create_proof(
        &params,
        &pk,
        std::slice::from_ref(&circuit),
        &[&[&public]],
        OsRng,
        &mut transcript,
    )?;
    let proof = transcript.finalize();
    let verify = |input: &[pallas::Base]| {
        let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(&proof[..]);
        verify_proof(
            &params,
            pk.get_vk(),
            SingleVerifier::new(&params),
            &[&[input]],
            &mut transcript,
        )
    };
    verify(&public)?;
    for index in [0, 18, 29, 59] {
        let mut wrong = public.clone();
        wrong[index] += pallas::Base::ONE;
        assert!(verify(&wrong).is_err());
    }
    let mut omitted = public.clone();
    omitted.drain(10..20);
    assert!(verify(&omitted).is_err());
    let summary = serde_json::json!({"scope":"six synthetic native outputs bound to SHA settlement rows in one proof", "k":K,"rows":6,"actions":6,"proof_bytes":proof.len(),"proof_verified":true,"changed_row_cmx_index_rejected":true,"omitted_row_rejected":true,"auction_plan_connected":false,"book_context_supplied":context_path.is_some(),"funding_proven":false,"native_custody_signature_verified":false,"classical_binding_assumption":true,"rseed_derivation_constrained":false,"ciphertext_constrained":false,"seconds":started.elapsed().as_secs_f64()});
    if let Some(directory) = export_directory {
        std::fs::write(directory.join("native-binding-proof"), &proof)?;
        std::fs::write(
            directory.join("native-proof-summary.json"),
            serde_json::to_vec_pretty(&summary)?,
        )?;
    }
    println!("{summary}");
    Ok(())
}
