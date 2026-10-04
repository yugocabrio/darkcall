//! Shared BN254 Poseidon2 row → native Ironwood cmx, including canonical point bytes.
//! Experimental/classical binding. ZIP 2005 rseed and ciphertext are not proved.
use darkcall_bn254_emulation::poseidon::{digest_words, pack_row, reference};
use darkcall_output_binding::sha::{Binding, DOMAIN, PoseidonBinding};
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

use std::time::Instant;
const K: u32 = 17;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let mut rng = OsRng;
    let key = loop {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        if let Some(key) = SpendingKey::from_bytes(bytes).into_option() {
            break key;
        }
    };
    let fvk = FullViewingKey::from(&key);
    let address = fvk.address_at(0u32, Scope::External);
    let mut builder = Builder::new(
        BundleType::DEFAULT,
        BundleVersion::ironwood_v3(),
        Flags::SPENDS_DISABLED,
        orchard::Anchor::empty_tree(),
    )
    .unwrap();
    builder
        .add_output(None, address, NoteValue::from_raw(50), [0; 512])
        .unwrap();
    let (bundle, meta) = builder.build::<i64>(&mut rng).unwrap().unwrap();
    let action_index = meta.output_action_index(0).unwrap();
    let (note, recipient, _) = bundle
        .decrypt_output_with_key(action_index, &fvk.to_ivk(Scope::External))
        .unwrap();
    assert_eq!(recipient, address);
    let gd = address.g_d();
    let pkd = address.pk_d().inner();
    let rho = note.rho();
    let psi = note.rseed().psi(&rho);
    let rcm = note
        .rseed()
        .rcm_v3(&rho, &gd, &pkd, note.value().inner(), &psi)
        .inner();
    let mut row = [0u8; 224];
    row[..16].copy_from_slice(DOMAIN);
    rng.fill_bytes(&mut row[16..112]); // synthetic conditions/root/ticket
    row[112..116].copy_from_slice(&0u32.to_le_bytes()); // refund role
    row[116..120].copy_from_slice(&(action_index as u32).to_le_bytes());
    row[120..152].copy_from_slice(gd.to_bytes().as_ref());
    row[152..184].copy_from_slice(pkd.to_bytes().as_ref());
    row[184..192].copy_from_slice(&50u64.to_le_bytes());
    rng.fill_bytes(&mut row[192..224]);
    let words = digest_words(&reference(&pack_row(&row)));
    let binding = Binding {
        gd: Value::known(gd.to_affine()),
        pkd: Value::known(pkd.to_affine()),
        value: Value::known(50),
        rho: Value::known(rho.into_inner()),
        psi: Value::known(psi),
        rcm: Value::known(rcm),
        prefix: std::array::from_fn(|i| {
            Value::known(u32::from_be_bytes(
                row[16 + i * 4..20 + i * 4].try_into().unwrap(),
            ))
        }),
        salt: std::array::from_fn(|i| {
            Value::known(u32::from_be_bytes(
                row[192 + i * 4..196 + i * 4].try_into().unwrap(),
            ))
        }),
    };
    let circuit = PoseidonBinding(binding);
    let cmx = bundle.actions()[action_index].cmx().to_bytes();
    let mut public: Vec<_> = words
        .into_iter()
        .map(|x| pallas::Base::from(x as u64))
        .collect();
    public.push(ExtractedNoteCommitment::from_bytes(&cmx).unwrap().inner());
    public.push(pallas::Base::from(action_index as u64));
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if args.len() != 2
            || args[0] != "--export-mpc-input"
            || args[1].is_empty()
            || !args[1]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("usage: poseidon_binding [--export-mpc-input LOCAL_FIXTURE_NAME]".into());
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let local = root.join(".local").canonicalize()?;
        let directory = local.join(&args[1]);
        std::fs::create_dir(&directory)?;
        std::fs::write(
            directory.join("Prover.toml"),
            format!("input = {}\n", serde_json::to_string(row.as_slice())?),
        )?;
        let digest_words = words;
        std::fs::write(
            directory.join("row-public.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"digest_words": digest_words, "action_index": action_index, "cmx": cmx}),
            )?,
        )?;
        eprintln!(
            "Synthetic MPC row input exported under ignored .local/{}",
            args[1]
        );
    }
    eprintln!("Poseidon/native mock started");
    MockProver::run(K, &circuit, vec![public.clone()])?.assert_satisfied();
    eprintln!("Poseidon/native mock passed; key generation started");
    let params: Params<vesta::Affine> = Params::new(K);
    let vk = keygen_vk(&params, &circuit.without_witnesses())?;
    let pk = keygen_pk(&params, vk, &circuit.without_witnesses())?;
    eprintln!("Poseidon/native proof generation started");
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
    for index in [0, 8, 9] {
        let mut changed = public.clone();
        changed[index] += pallas::Base::ONE;
        assert!(verify(&changed).is_err());
    }
    let wrong_amount = PoseidonBinding(Binding {
        value: Value::known(51),
        ..circuit.0.clone()
    });
    assert!(
        MockProver::run(K, &wrong_amount, vec![public.clone()])?
            .verify()
            .is_err()
    );
    let wrong_recipient = PoseidonBinding(Binding {
        gd: Value::known((-*gd).to_affine()),
        ..circuit.0
    });
    assert!(
        MockProver::run(K, &wrong_recipient, vec![public])?
            .verify()
            .is_err()
    );
    println!(
        "{}",
        serde_json::json!({
            "scope": "BN254 Poseidon2 settlement row binds canonical recipient/amount to native Ironwood cmx",
            "k": K, "row_bytes": 224, "proof_bytes": proof.len(), "proof_verified": true,
            "canonical_point_encoding": true, "native_output_binding": true,
            "auction_plan_connected": false, "classical_binding_assumption": true,
            "rseed_derivation_constrained": false, "ciphertext_constrained": false,
            "changed_hash_cmx_index_rejected": true, "changed_amount_recipient_rejected": true,
            "seconds": start.elapsed().as_secs_f64(),
        })
    );
    Ok(())
}
