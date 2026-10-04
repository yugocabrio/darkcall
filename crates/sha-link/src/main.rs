//! Actual Halo2 proof of the shared 224-byte SHA-256 statement.
use darkcall_sha_link::{Cell, Config};
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        Circuit, Column, ConstraintSystem, Error, Instance, SingleVerifier, create_proof,
        keygen_pk, keygen_vk, verify_proof,
    },
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite, Challenge255},
};
use pasta_curves::{pallas, vesta};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use std::time::Instant;

const K: u32 = 17;
#[derive(Clone)]
struct ShaConfig {
    chip: Config,
    instance: Column<Instance>,
}
#[derive(Clone)]
struct ShaCircuit {
    words: [Value<u32>; 56],
}
impl Default for ShaCircuit {
    fn default() -> Self {
        Self {
            words: [Value::unknown(); 56],
        }
    }
}
impl Circuit<pallas::Base> for ShaCircuit {
    type Config = ShaConfig;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self::default()
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> ShaConfig {
        let chip = Config::configure(meta);
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        ShaConfig { chip, instance }
    }
    fn synthesize(
        &self,
        config: ShaConfig,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        config.chip.load(&mut layouter)?;
        let mut words = Vec::new();
        for word in self.words {
            words.push(config.chip.word(&mut layouter, word)?);
        }
        let words: [Cell; 56] = words.try_into().unwrap();
        let digest = config.chip.hash_224(&mut layouter, &words)?;
        for (i, cell) in digest.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), config.instance, i)?;
        }
        Ok(())
    }
}

fn fixture(bytes: &[u8; 224]) -> (ShaCircuit, Vec<pallas::Base>) {
    let circuit = ShaCircuit {
        words: std::array::from_fn(|i| {
            Value::known(u32::from_be_bytes(
                bytes[i * 4..i * 4 + 4].try_into().unwrap(),
            ))
        }),
    };
    let digest = Sha256::digest(bytes);
    let public = digest
        .chunks_exact(4)
        .map(|x| pallas::Base::from(u32::from_be_bytes(x.try_into().unwrap()) as u64))
        .collect();
    (circuit, public)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let mut bytes = [0u8; 224];
    OsRng.fill_bytes(&mut bytes);
    let (circuit, public) = fixture(&bytes);
    MockProver::run(K, &circuit, vec![public.clone()])?.assert_satisfied();
    let params: Params<vesta::Affine> = Params::new(K);
    let vk = keygen_vk(&params, &circuit.without_witnesses())?;
    let pk = keygen_pk(&params, vk, &circuit.without_witnesses())?;
    let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(Vec::new());
    create_proof(
        &params,
        &pk,
        &[circuit],
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
    let mut altered = public.clone();
    altered[0] += pallas::Base::from(1);
    assert!(verify(&altered).is_err());
    println!(
        "{}",
        serde_json::json!({
            "scope": "standard SHA-256 with linked Halo2 message/state/feed-forward cells",
            "private_bytes": 224, "compression_blocks": 4, "k": K,
            "proof_bytes": proof.len(), "proof_verified": true,
            "rust_sha256_matches": true, "altered_digest_rejected": true,
            "native_output_binding": false, "seconds": start.elapsed().as_secs_f64(),
        })
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha256_vectors_and_changed_input() {
        for bytes in [[0; 224], std::array::from_fn(|i| i as u8)] {
            let (circuit, public) = fixture(&bytes);
            MockProver::run(K, &circuit, vec![public.clone()])
                .unwrap()
                .assert_satisfied();
            let mut changed = bytes;
            changed[0] ^= 1;
            let (bad, _) = fixture(&changed);
            assert!(
                MockProver::run(K, &bad, vec![public])
                    .unwrap()
                    .verify()
                    .is_err()
            );
        }
    }
}
