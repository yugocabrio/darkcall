//! Actual arithmetic proof before measuring any complete hash. Public synthetic
//! limbs only; no native output or auction connection claimed.
use darkcall_bn254_emulation::{ArithmeticGate, MODULUS, arithmetic_public};
use ff::Field;
use halo2_proofs::{
    circuit::Value,
    plonk::{Circuit, SingleVerifier, create_proof, keygen_pk, keygen_vk, verify_proof},
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite, Challenge255},
};
use num_bigint::BigUint;
use pasta_curves::{pallas, vesta};
use rand::rngs::OsRng;
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let a = &*MODULUS - BigUint::from(2u64);
    let b = &*MODULUS - BigUint::from(3u64);
    let circuit = ArithmeticGate {
        a: Value::known(a.clone()),
        b: Value::known(b.clone()),
    };
    let public = arithmetic_public(&a, &b);
    let params: Params<vesta::Affine> = Params::new(10);
    let vk = keygen_vk(&params, &circuit.without_witnesses())?;
    let pk = keygen_pk(&params, vk, &circuit.without_witnesses())?;
    let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(Vec::new());
    let proof_started = Instant::now();
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
    changed[12] += pallas::Base::ONE;
    assert!(verify(&changed).is_err());
    println!(
        "{}",
        serde_json::json!({"scope":"canonical BN254 addition/multiplication constrained on Pallas",
        "k":10,"proof_bytes":proof.len(),"proof_verified":true,"changed_product_rejected":true,
        "full_poseidon2_hash":false,"native_output_binding":false,"public_synthetic_limbs":true,
        "proof_seconds":proof_seconds,"total_seconds":started.elapsed().as_secs_f64()})
    );
    Ok(())
}
