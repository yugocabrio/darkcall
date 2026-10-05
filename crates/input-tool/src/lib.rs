//! Exact host encoding of the pinned Noir protocol_lib Poseidon2 sponge.
//! This computes public commitments; it does not verify ZK proofs or funding.
use ark_bn254::Fr;
use ark_ff::{BigInteger, Field, PrimeField};
use mpc_core::gadgets::poseidon2::{POSEIDON2_BN254_T4_PARAMS, Poseidon2};

pub fn hash(input: &[Fr]) -> Fr {
    let permutation = Poseidon2::<Fr, 4, 5>::new(&POSEIDON2_BN254_T4_PARAMS);
    let mut state = [Fr::from(0); 4];
    state[3] = Fr::from(input.len() as u64) * Fr::from(2).pow([64]);
    for chunk in input.chunks_exact(3) {
        for i in 0..3 {
            state[i] += chunk[i];
        }
        permutation.permutation_in_place(&mut state);
    }
    let remainder = input.chunks_exact(3).remainder();
    for (i, value) in remainder.iter().enumerate() {
        state[i] += value;
    }
    permutation.permutation(&state)[0]
}
pub fn domain(text: &[u8]) -> Fr {
    assert!(text.len() < 31);
    Fr::from_be_bytes_mod_order(text)
}
pub fn limbs(bytes: &[u8; 32]) -> [Fr; 4] {
    std::array::from_fn(|i| {
        Fr::from(u64::from_be_bytes(
            bytes[i * 8..(i + 1) * 8].try_into().unwrap(),
        ))
    })
}
pub fn field_bytes(value: Fr) -> [u8; 32] {
    let encoded = value.into_bigint().to_bytes_be();
    let mut bytes = [0; 32];
    bytes[32 - encoded.len()..].copy_from_slice(&encoded);
    bytes
}
pub fn canonical_field(bytes: &[u8; 32]) -> Result<Fr, String> {
    let value = Fr::from_be_bytes_mod_order(bytes);
    if field_bytes(value) != *bytes {
        return Err("noncanonical BN254 field encoding".into());
    }
    Ok(value)
}
pub fn decimal_field_bytes(text: &str) -> Result<[u8; 32], String> {
    use std::str::FromStr;
    let value = Fr::from_str(text).map_err(|_| "invalid decimal BN254 field")?;
    if value.to_string() != text {
        return Err("noncanonical decimal BN254 field".into());
    }
    Ok(field_bytes(value))
}
pub fn context(conditions: &[u8; 32], units: u32, reserve: u64, budget: u64, fee: u64) -> Fr {
    let mut fields = vec![domain(b"DarkCall/ctx/v1")];
    fields.extend(limbs(conditions));
    fields.extend([
        Fr::from(units),
        Fr::from(reserve),
        Fr::from(budget),
        Fr::from(fee),
    ]);
    hash(&fields)
}
pub fn root(context: Fr, active: [bool; 4], commitments: [Fr; 4]) -> Fr {
    let mut fields = vec![
        domain(b"DarkCall/accepted/v1"),
        context,
        Fr::from(4),
        Fr::from(active.iter().filter(|x| **x).count() as u64),
    ];
    fields.extend(active.map(|x| Fr::from(u64::from(x))));
    fields.extend(commitments);
    hash(&fields)
}
pub fn four_active_root(
    conditions: &[u8; 32],
    units: u32,
    reserve: u64,
    budget: u64,
    fee: u64,
    commitments: [[u8; 32]; 4],
) -> Result<[u8; 32], String> {
    let fields = commitments.each_ref().map(canonical_field);
    let fields = fields.into_iter().collect::<Result<Vec<_>, _>>()?;
    Ok(field_bytes(root(
        context(conditions, units, reserve, budget, fee),
        [true; 4],
        fields.try_into().map_err(|_| "commitment count")?,
    )))
}
