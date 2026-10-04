//! Full existing BN254 Poseidon2 sponge on Pallas; hash-only comparison, not
//! yet linked to native NoteCommit or canonical source cells.
use darkcall_bn254_emulation::poseidon::{HashGate, digest_words, pack_row, reference};
use ff::Field;
use halo2_proofs::{
    circuit::Value,
    dev::MockProver,
    plonk::{Circuit, SingleVerifier, create_proof, keygen_pk, keygen_vk, verify_proof},
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite, Challenge255},
};
use pasta_curves::{pallas, vesta};
use rand::{RngCore, rngs::OsRng};
use std::time::Instant;
const K: u32 = 17;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut row = [0u8; 224];
    OsRng.fill_bytes(&mut row);
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if args.len() != 2 || !["--row-json", "--reference-row-json"].contains(&args[0].as_str()) {
            return Err("usage: poseidon_gate [--row-json LOCAL_ROW_FILE]".into());
        }
        let file = std::path::Path::new(&args[1]).canonicalize()?;
        let local = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(".local")
            .canonicalize()?;
        if !file.starts_with(local) || std::fs::metadata(&file)?.len() > 8192 {
            return Err("expected a small ignored synthetic row file".into());
        }
        let bytes: Vec<u8> = serde_json::from_slice(&std::fs::read(file)?)?;
        row = bytes.try_into().map_err(|_| "expected 224 bytes")?;
    }
    let inputs = pack_row(&row);
    let words = digest_words(&reference(&inputs));
    if args.first().map(String::as_str) == Some("--reference-row-json") {
        println!(
            "{}",
            serde_json::json!({"digest_words":words,"reference_only":true})
        );
        return Ok(());
    }
    let circuit = HashGate {
        inputs: inputs.map(Value::known),
    };
    let public = words.map(|w| pallas::Base::from(w as u64)).to_vec();
    let mock_started = Instant::now();
    eprintln!("Non-native Poseidon2 mock started");
    MockProver::run(K, &circuit, vec![public.clone()])?.assert_satisfied();
    let mock_seconds = mock_started.elapsed().as_secs_f64();
    eprintln!("Non-native Poseidon2 mock passed; keygen started");
    let key_started = Instant::now();
    let params: Params<vesta::Affine> = Params::new(K);
    let vk = keygen_vk(&params, &circuit.without_witnesses())?;
    let pk = keygen_pk(&params, vk, &circuit.without_witnesses())?;
    let key_seconds = key_started.elapsed().as_secs_f64();
    eprintln!("Non-native Poseidon2 proof started");
    let proof_started = Instant::now();
    let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(Vec::new());
    create_proof(
        &params,
        &pk,
        std::slice::from_ref(&circuit),
        &[&[&public]],
        OsRng,
        &mut transcript,
    )?;
    let proof_seconds = proof_started.elapsed().as_secs_f64();
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
    let mut changed = public.clone();
    changed[0] += pallas::Base::ONE;
    assert!(verify(&changed).is_err());
    println!(
        "{}",
        serde_json::json!({"scope":"exact existing BN254 Poseidon2 sponge on Pallas, eight private elements",
        "k":K,"proof_verified":true,"changed_digest_rejected":true,"proof_bytes":proof.len(),"digest_words":words,
        "mock_seconds":mock_seconds,"keygen_seconds":key_seconds,"proof_seconds":proof_seconds,"total_seconds":started.elapsed().as_secs_f64(),
        "native_output_binding":false,"canonical_native_bytes_linked":false,"comparison_only":true})
    );
    Ok(())
}
