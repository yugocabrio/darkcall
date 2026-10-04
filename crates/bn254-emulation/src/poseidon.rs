//! Exact existing coNoir BN254 Poseidon2 parameters and sponge semantics.
//! This experiment hashes eight canonical private elements; native byte/cell
//! binding is not supplied by this standalone hash circuit.
use crate::{Config, Element};
use ark_bn254::Fr;
use ark_ff::{BigInteger, Field, PrimeField};
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    plonk::{Circuit, Column, ConstraintSystem, Error, Instance},
};
use mpc_core::gadgets::poseidon2::{POSEIDON2_BN254_T4_PARAMS, Poseidon2};
use num_bigint::BigUint;
use pasta_curves::pallas;

fn integer(value: Fr) -> BigUint {
    BigUint::from_bytes_le(&value.into_bigint().to_bytes_le())
}

pub fn reference(inputs: &[BigUint; 8]) -> BigUint {
    let permutation = Poseidon2::<Fr, 4, 5>::new(&POSEIDON2_BN254_T4_PARAMS);
    let mut state = [Fr::from(0); 4];
    state[3] = Fr::from(8) * Fr::from(2).pow([64]);
    for chunk in inputs.chunks_exact(3) {
        for i in 0..3 {
            state[i] += Fr::from_be_bytes_mod_order(&chunk[i].to_bytes_be());
        }
        permutation.permutation_in_place(&mut state);
    }
    for (i, value) in inputs.chunks_exact(3).remainder().iter().enumerate() {
        state[i] += Fr::from_be_bytes_mod_order(&value.to_bytes_be());
    }
    integer(permutation.permutation(&state)[0])
}
pub fn pack_row(bytes: &[u8; 224]) -> [BigUint; 8] {
    std::array::from_fn(|i| BigUint::from_bytes_be(&bytes[i * 30..((i + 1) * 30).min(224)]))
}
pub fn digest_words(digest: &BigUint) -> [u32; 8] {
    let words = digest.to_u32_digits();
    std::array::from_fn(|i| words.get(7 - i).copied().unwrap_or(0))
}
impl Config {
    fn external(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        s: &[Element; 4],
    ) -> Result<[Element; 4], Error> {
        let t0 = self.add(layouter, &s[0], &s[1])?;
        let t1 = self.add(layouter, &s[2], &s[3])?;
        let two_b = self.add(layouter, &s[1], &s[1])?;
        let two_d = self.add(layouter, &s[3], &s[3])?;
        let t2 = self.add(layouter, &two_b, &t1)?;
        let t3 = self.add(layouter, &two_d, &t0)?;
        let two_t1 = self.add(layouter, &t1, &t1)?;
        let four_t1 = self.add(layouter, &two_t1, &two_t1)?;
        let two_t0 = self.add(layouter, &t0, &t0)?;
        let four_t0 = self.add(layouter, &two_t0, &two_t0)?;
        let t4 = self.add(layouter, &four_t1, &t3)?;
        let t5 = self.add(layouter, &four_t0, &t2)?;
        Ok([
            self.add(layouter, &t3, &t5)?,
            t5,
            self.add(layouter, &t2, &t4)?,
            t4,
        ])
    }
    fn fifth(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        x: &Element,
    ) -> Result<Element, Error> {
        let square = self.multiply(layouter, x, x)?;
        let fourth = self.multiply(layouter, &square, &square)?;
        self.multiply(layouter, x, &fourth)
    }
    pub fn permutation(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        input: &[Element; 4],
    ) -> Result<[Element; 4], Error> {
        let params = &*POSEIDON2_BN254_T4_PARAMS;
        let mut state = self.external(layouter, input)?;
        for r in 0..params.rounds_f_beginning {
            for (i, word) in state.iter_mut().enumerate() {
                let constant =
                    self.constant(layouter, integer(params.round_constants_external[r][i]))?;
                let x = self.add(layouter, word, &constant)?;
                *word = self.fifth(layouter, &x)?;
            }
            state = self.external(layouter, &state)?;
        }
        for r in 0..params.rounds_p {
            let constant = self.constant(layouter, integer(params.round_constants_internal[r]))?;
            let x = self.add(layouter, &state[0], &constant)?;
            state[0] = self.fifth(layouter, &x)?;
            let mut sum = self.add(layouter, &state[0], &state[1])?;
            sum = self.add(layouter, &sum, &state[2])?;
            sum = self.add(layouter, &sum, &state[3])?;
            for (i, word) in state.iter_mut().enumerate() {
                let constant = self.constant(layouter, integer(params.mat_internal_diag_m_1[i]))?;
                let product = self.multiply(layouter, word, &constant)?;
                *word = self.add(layouter, &product, &sum)?;
            }
        }
        for r in params.rounds_f_beginning..params.rounds_f_beginning + params.rounds_f_end {
            for (i, word) in state.iter_mut().enumerate() {
                let constant =
                    self.constant(layouter, integer(params.round_constants_external[r][i]))?;
                let x = self.add(layouter, word, &constant)?;
                *word = self.fifth(layouter, &x)?;
            }
            state = self.external(layouter, &state)?;
        }
        Ok(state)
    }
    pub fn hash_eight(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        inputs: &[Element; 8],
    ) -> Result<Element, Error> {
        let zero = self.constant(layouter, BigUint::from(0u32))?;
        let capacity = self.constant(layouter, BigUint::from(8u32) << 64)?;
        let mut state = [zero.clone(), zero.clone(), zero, capacity];
        for chunk in inputs.chunks_exact(3) {
            for i in 0..3 {
                state[i] = self.add(layouter, &state[i], &chunk[i])?;
            }
            state = self.permutation(layouter, &state)?;
        }
        for (i, word) in inputs.chunks_exact(3).remainder().iter().enumerate() {
            state[i] = self.add(layouter, &state[i], word)?;
        }
        Ok(self.permutation(layouter, &state)?[0].clone())
    }
}
#[derive(Clone)]
pub struct HashGate {
    pub inputs: [Value<BigUint>; 8],
}
impl Circuit<pallas::Base> for HashGate {
    type Config = (Config, Column<Instance>);
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self {
            inputs: std::array::from_fn(|_| Value::unknown()),
        }
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
        let config = Config::configure(meta);
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        (config, instance)
    }
    fn synthesize(
        &self,
        (config, instance): Self::Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        config.load(&mut layouter)?;
        let mut inputs = Vec::new();
        for value in &self.inputs {
            inputs.push(config.assign(&mut layouter, value.clone())?);
        }
        let inputs = inputs.try_into().map_err(|_| Error::Synthesis)?;
        let digest = config.hash_eight(&mut layouter, &inputs)?;
        for (i, cell) in config.words(&mut layouter, &digest)?.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), instance, i)?;
        }
        Ok(())
    }
}
