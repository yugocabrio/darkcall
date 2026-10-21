//! Times the fixed K=17 Pasta parameters: generation (checked against the pinned
//! digest), reading them from the checked cache, and the shared18 verifying key.
use darkcall_output_binding::{
    poseidon_rows::{K, PARAMS_SHA256, fixed_params},
    sha::SharedPoseidonBinding,
};
use halo2_proofs::{plonk::keygen_vk, poly::commitment::Params};
use pasta_curves::vesta;
use sha2::{Digest, Sha256};
use std::time::Instant;

fn main() {
    let t = Instant::now();
    let computed: Params<vesta::Affine> = Params::new(K);
    let new_seconds = t.elapsed().as_secs_f64();
    let mut bytes = Vec::new();
    computed.write(&mut bytes).unwrap();
    let t = Instant::now();
    let (fixed, cached) = fixed_params().unwrap();
    let fixed_seconds = t.elapsed().as_secs_f64();
    let mut again = Vec::new();
    fixed.write(&mut again).unwrap();
    let t = Instant::now();
    let _vk = keygen_vk(&fixed, &SharedPoseidonBinding::default()).unwrap();
    let vk_seconds = t.elapsed().as_secs_f64();
    println!(
        "{}",
        serde_json::json!({"k":K,"new_seconds":new_seconds,"bytes":bytes.len(),
        "computed_sha256_matches_pin":format!("{:x}", Sha256::digest(&bytes)) == PARAMS_SHA256,
        "fixed_params_seconds":fixed_seconds,"fixed_params_from_cache":cached,
        "fixed_equals_computed":again == bytes,"vk_seconds":vk_seconds})
    );
}
