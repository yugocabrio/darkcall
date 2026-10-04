use darkcall_sha_link::{Cell, Config};
use ff::{Field, PrimeField};
use group::{Curve, Group, GroupEncoding};
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance},
};
use pasta_curves::{arithmetic::CurveAffine, pallas};
use rand::rngs::OsRng;

#[derive(Clone)]
struct TestConfig {
    chip: Config,
    advice: Column<Advice>,
    public: Column<Instance>,
}
fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> TestConfig {
    let chip = Config::configure(meta);
    let advice = meta.advice_column();
    meta.enable_equality(advice);
    let public = meta.instance_column();
    meta.enable_equality(public);
    TestConfig {
        chip,
        advice,
        public,
    }
}
fn field(
    layouter: &mut impl Layouter<pallas::Base>,
    col: Column<Advice>,
    value: Value<pallas::Base>,
) -> Result<Cell, Error> {
    layouter.assign_region(
        || "native field",
        |mut region| region.assign_advice(|| "field", col, 0, || value),
    )
}
#[derive(Clone)]
struct Encoding {
    field: Value<pallas::Base>,
    words: [Value<u32>; 8],
}
impl Circuit<pallas::Base> for Encoding {
    type Config = TestConfig;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self {
            field: Value::unknown(),
            words: [Value::unknown(); 8],
        }
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> TestConfig {
        configure(meta)
    }
    fn synthesize(
        &self,
        config: TestConfig,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        config.chip.load_bytes(&mut layouter)?;
        let cell = field(&mut layouter, config.advice, self.field)?;
        let mut words = Vec::new();
        for word in self.words {
            words.push(config.chip.word(&mut layouter, word)?);
        }
        config
            .chip
            .assert_canonical_field(&mut layouter, &cell, &words.try_into().unwrap())
    }
}
#[test]
fn rejects_modulus_alias_and_wrong_coordinate() {
    let good = Encoding {
        field: Value::known(pallas::Base::ZERO),
        words: [Value::known(0); 8],
    };
    MockProver::run(11, &good, vec![vec![]])
        .unwrap()
        .assert_satisfied();
    // p maps to zero in the field, but is not the canonical encoding of zero.
    let mut modulus = (-pallas::Base::ONE).to_repr();
    modulus[0] += 1;
    let alias = Encoding {
        field: good.field,
        words: std::array::from_fn(|i| {
            Value::known(u32::from_le_bytes(
                modulus[i * 4..i * 4 + 4].try_into().unwrap(),
            ))
        }),
    };
    assert!(
        MockProver::run(11, &alias, vec![vec![]])
            .unwrap()
            .verify()
            .is_err()
    );
    let wrong = Encoding {
        field: Value::known(pallas::Base::ONE),
        ..good
    };
    assert!(
        MockProver::run(11, &wrong, vec![vec![]])
            .unwrap()
            .verify()
            .is_err()
    );
}
#[derive(Clone)]
struct Point {
    x: Value<pallas::Base>,
    y: Value<pallas::Base>,
}
impl Circuit<pallas::Base> for Point {
    type Config = TestConfig;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self {
            x: Value::unknown(),
            y: Value::unknown(),
        }
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> TestConfig {
        configure(meta)
    }
    fn synthesize(
        &self,
        config: TestConfig,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        config.chip.load_bytes(&mut layouter)?;
        let x = field(&mut layouter, config.advice, self.x)?;
        let y = field(&mut layouter, config.advice, self.y)?;
        for (i, word) in config
            .chip
            .compressed_point(&mut layouter, &x, &y)?
            .iter()
            .enumerate()
        {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
#[test]
fn canonical_compression_matches_pasta_encoding_and_rejects_sign_change() {
    for point in [pallas::Point::generator(), pallas::Point::random(OsRng)] {
        let affine = point.to_affine();
        let coordinates = affine.coordinates().unwrap();
        let circuit = Point {
            x: Value::known(*coordinates.x()),
            y: Value::known(*coordinates.y()),
        };
        let public: Vec<_> = affine
            .to_bytes()
            .chunks_exact(4)
            .map(|x| pallas::Base::from(u32::from_be_bytes(x.try_into().unwrap()) as u64))
            .collect();
        MockProver::run(11, &circuit, vec![public.clone()])
            .unwrap()
            .assert_satisfied();
        let wrong_sign = Point {
            y: Value::known(-*coordinates.y()),
            ..circuit
        };
        assert!(
            MockProver::run(11, &wrong_sign, vec![public])
                .unwrap()
                .verify()
                .is_err()
        );
    }
}
