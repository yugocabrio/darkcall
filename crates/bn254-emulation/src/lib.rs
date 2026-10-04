//! Experimental canonical BN254 scalar arithmetic over Pallas, not yet a hash
//! or native output binding. Integer constraints are bounded below Pallas p.
use ff::{Field, PrimeField};
use halo2_proofs::{
    circuit::{AssignedCell, Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, Selector, TableColumn},
    poly::Rotation,
};
use num_bigint::{BigInt, BigUint};
use num_traits::{One, ToPrimitive};
use pasta_curves::pallas;
use std::sync::LazyLock;
pub mod poseidon;

pub static MODULUS: LazyLock<BigUint> = LazyLock::new(|| {
    BigUint::parse_bytes(
        b"30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001",
        16,
    )
    .unwrap()
});
pub type Cell = AssignedCell<pallas::Base, pallas::Base>;
const OFFSET: u128 = 1u128 << 71;
fn limb_values(value: &BigUint) -> [u64; 4] {
    let words = value.to_u64_digits();
    std::array::from_fn(|i| words.get(i).copied().unwrap_or(0))
}
fn fp(value: &BigInt) -> pallas::Base {
    let magnitude = value
        .magnitude()
        .to_u128()
        .expect("bounded integer witness");
    let field = pallas::Base::from_u128(magnitude);
    if value.sign() == num_bigint::Sign::Minus {
        -field
    } else {
        field
    }
}

#[derive(Clone)]
pub struct Element {
    pub limbs: [Cell; 4],
    pub value: Value<BigUint>,
}
#[derive(Clone)]
pub struct Config {
    columns: [Column<Advice>; 18],
    range32: Selector,
    range64: Selector,
    range72: Selector,
    canonical: Selector,
    multiplication: [Selector; 8],
    addition: [Selector; 4],
    split: Selector,
    pack: Selector,
    table: TableColumn,
}
impl Config {
    pub fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self {
        let columns = std::array::from_fn(|_| meta.advice_column());
        for col in columns {
            meta.enable_equality(col);
        }
        let fixed = meta.fixed_column();
        meta.enable_constant(fixed);
        let table = meta.lookup_table_column();
        let range32 = meta.complex_selector();
        let range64 = meta.complex_selector();
        let range72 = meta.complex_selector();
        for (i, col) in columns.iter().enumerate().take(10).skip(1) {
            meta.lookup(|meta| {
                let q = if i <= 4 {
                    meta.query_selector(range32)
                        + meta.query_selector(range64)
                        + meta.query_selector(range72)
                } else if i == 9 {
                    meta.query_selector(range72)
                } else {
                    meta.query_selector(range64) + meta.query_selector(range72)
                };
                vec![(q * meta.query_advice(*col, Rotation::cur()), table)]
            });
        }
        meta.create_gate("bounded 64/72-bit integer", |meta| {
            let value = meta.query_advice(columns[0], Rotation::cur());
            let mut sum = Expression::Constant(pallas::Base::ZERO);
            for (i, col) in columns[1..=8].iter().enumerate() {
                sum = sum
                    + meta.query_advice(*col, Rotation::cur())
                        * pallas::Base::from_u128(1u128 << (8 * i));
            }
            let mut low_sum = Expression::Constant(pallas::Base::ZERO);
            for (i, col) in columns[1..=4].iter().enumerate() {
                low_sum = low_sum
                    + meta.query_advice(*col, Rotation::cur())
                        * pallas::Base::from(1u64 << (8 * i));
            }
            let q32 = meta.query_selector(range32);
            let q64 = meta.query_selector(range64);
            let q72 = meta.query_selector(range72);
            vec![
                q32 * (value.clone() - low_sum),
                q64 * (value.clone() - sum.clone()),
                q72 * (value
                    - sum
                    - meta.query_advice(columns[9], Rotation::cur())
                        * pallas::Base::from_u128(1u128 << 64)),
            ]
        });
        let canonical = meta.selector();
        meta.create_gate("canonical BN254 subtraction", |meta| {
            let q = meta.query_selector(canonical);
            let x = meta.query_advice(columns[0], Rotation::cur());
            let bound = meta.query_advice(columns[1], Rotation::cur());
            let diff = meta.query_advice(columns[2], Rotation::cur());
            let previous = meta.query_advice(columns[3], Rotation::cur());
            let next = meta.query_advice(columns[4], Rotation::cur());
            vec![
                q.clone()
                    * (bound - x - previous + next.clone() * pallas::Base::from_u128(1u128 << 64)
                        - diff),
                q * next.clone() * (next - Expression::Constant(pallas::Base::ONE)),
            ]
        });
        let multiplication = std::array::from_fn(|_| meta.selector());
        let modulus = limb_values(&MODULUS);
        for k in 0..8 {
            meta.create_gate("exact integer product limb", |meta| {
                let q = meta.query_selector(multiplication[k]);
                let mut relation = Expression::Constant(pallas::Base::ZERO);
                for i in 0..4 {
                    for j in 0..4 {
                        if i + j == k {
                            relation = relation
                                + meta.query_advice(columns[i], Rotation::cur())
                                    * meta.query_advice(columns[4 + j], Rotation::cur())
                                - meta.query_advice(columns[8 + i], Rotation::cur())
                                    * pallas::Base::from(modulus[j]);
                        }
                    }
                }
                if k < 4 {
                    relation = relation - meta.query_advice(columns[12 + k], Rotation::cur());
                }
                let offset = Expression::Constant(pallas::Base::from_u128(OFFSET));
                relation = relation + meta.query_advice(columns[16], Rotation::cur())
                    - offset.clone()
                    - (meta.query_advice(columns[17], Rotation::cur()) - offset)
                        * pallas::Base::from_u128(1u128 << 64);
                vec![q * relation]
            });
        }
        let addition = std::array::from_fn(|_| meta.selector());
        for k in 0..4 {
            meta.create_gate("exact modular addition limb", |meta| {
                let q = meta.query_selector(addition[k]);
                let a = meta.query_advice(columns[0], Rotation::cur());
                let b = meta.query_advice(columns[1], Rotation::cur());
                let r = meta.query_advice(columns[2], Rotation::cur());
                let quotient = meta.query_advice(columns[3], Rotation::cur());
                let previous = meta.query_advice(columns[4], Rotation::cur())
                    - Expression::Constant(pallas::Base::from_u128(OFFSET));
                let next = meta.query_advice(columns[5], Rotation::cur())
                    - Expression::Constant(pallas::Base::from_u128(OFFSET));
                vec![
                    q.clone()
                        * (a + b - r - quotient.clone() * pallas::Base::from(modulus[k])
                            + previous
                            - next * pallas::Base::from_u128(1u128 << 64)),
                    q * quotient.clone() * (quotient - Expression::Constant(pallas::Base::ONE)),
                ]
            });
        }
        let split = meta.selector();
        meta.create_gate("split u64 into u32 words", |meta| {
            let q = meta.query_selector(split);
            vec![
                q * (meta.query_advice(columns[0], Rotation::cur())
                    - meta.query_advice(columns[1], Rotation::cur())
                    - meta.query_advice(columns[2], Rotation::cur())
                        * pallas::Base::from(1u64 << 32)),
            ]
        });
        let pack = meta.selector();
        meta.create_gate("packed source bytes equal canonical limb", |meta| {
            let mut sum = Expression::Constant(pallas::Base::ZERO);
            for (i, col) in columns[1..=8].iter().enumerate() {
                sum = sum
                    + meta.query_advice(*col, Rotation::cur())
                        * pallas::Base::from(1u64 << (8 * i));
            }
            vec![meta.query_selector(pack) * (meta.query_advice(columns[0], Rotation::cur()) - sum)]
        });
        Self {
            columns,
            range32,
            range64,
            range72,
            canonical,
            multiplication,
            addition,
            split,
            pack,
            table,
        }
    }
    pub fn load(&self, layouter: &mut impl Layouter<pallas::Base>) -> Result<(), Error> {
        layouter.assign_table(
            || "bytes",
            |mut table| {
                for i in 0..256 {
                    table.assign_cell(
                        || "byte",
                        self.table,
                        i,
                        || Value::known(pallas::Base::from(i as u64)),
                    )?;
                }
                Ok(())
            },
        )
    }
    fn range(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        value: Value<BigInt>,
        bits: usize,
    ) -> Result<Cell, Error> {
        layouter.assign_region(
            || "range bounded integer",
            |mut region| {
                (if bits == 32 {
                    self.range32
                } else if bits == 64 {
                    self.range64
                } else {
                    self.range72
                })
                .enable(&mut region, 0)?;
                let cell = region.assign_advice(
                    || "integer",
                    self.columns[0],
                    0,
                    || value.as_ref().map(fp),
                )?;
                for i in 0..9 {
                    region.assign_advice(
                        || "byte",
                        self.columns[i + 1],
                        0,
                        || {
                            if i >= bits / 8 {
                                Value::known(pallas::Base::ZERO)
                            } else {
                                value.as_ref().map(|v| {
                                    let digit = (v.magnitude() >> (8 * i)) & BigUint::from(255u32);
                                    pallas::Base::from(digit.to_u64().unwrap())
                                })
                            }
                        },
                    )?;
                }
                Ok(cell)
            },
        )
    }
    pub fn assign(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        value: Value<BigUint>,
    ) -> Result<Element, Error> {
        let words = value.as_ref().map(limb_values);
        let mut cells = Vec::new();
        for i in 0..4 {
            cells.push(self.range(layouter, words.map(|w| BigInt::from(w[i])), 64)?);
        }
        let limbs: [Cell; 4] = cells.try_into().map_err(|_| Error::Synthesis)?;
        let bound = limb_values(&(&*MODULUS - BigUint::one()));
        let mut previous_value = Value::known(0u64);
        let mut previous_cell = None;
        for i in 0..4 {
            let subtraction = words.zip(previous_value).map(|(w, previous)| {
                let raw = bound[i] as i128 - w[i] as i128 - previous as i128;
                let next = u64::from(raw < 0);
                ((raw + ((next as i128) << 64)) as u64, next)
            });
            let difference = self.range(layouter, subtraction.map(|(v, _)| BigInt::from(v)), 64)?;
            let next = layouter.assign_region(
                || "canonical reduction",
                |mut region| {
                    self.canonical.enable(&mut region, 0)?;
                    limbs[i].copy_advice(|| "limb", &mut region, self.columns[0], 0)?;
                    region.assign_advice_from_constant(
                        || "bound",
                        self.columns[1],
                        0,
                        pallas::Base::from(bound[i]),
                    )?;
                    difference.copy_advice(|| "difference", &mut region, self.columns[2], 0)?;
                    if let Some(previous) = &previous_cell {
                        let previous: &Cell = previous;
                        previous.copy_advice(
                            || "previous borrow",
                            &mut region,
                            self.columns[3],
                            0,
                        )?;
                    } else {
                        region.assign_advice_from_constant(
                            || "initial borrow",
                            self.columns[3],
                            0,
                            pallas::Base::ZERO,
                        )?;
                    }
                    let next = region.assign_advice(
                        || "next borrow",
                        self.columns[4],
                        0,
                        || subtraction.map(|(_, v)| pallas::Base::from(v)),
                    )?;
                    if i == 3 {
                        region.constrain_constant(next.cell(), pallas::Base::ZERO)?;
                    }
                    Ok(next)
                },
            )?;
            previous_value = subtraction.map(|(_, v)| v);
            previous_cell = Some(next);
        }
        Ok(Element { limbs, value })
    }

    /// Canonical digest limbs to eight u32 words, big-endian word order.
    pub fn words(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        element: &Element,
    ) -> Result<[Cell; 8], Error> {
        let values = element.value.as_ref().map(limb_values);
        let mut result = Vec::new();
        for i in 0..4 {
            let low = self.range(layouter, values.map(|w| BigInt::from(w[i] as u32)), 32)?;
            let high = self.range(
                layouter,
                values.map(|w| BigInt::from((w[i] >> 32) as u32)),
                32,
            )?;
            layouter.assign_region(
                || "digest word split",
                |mut region| {
                    self.split.enable(&mut region, 0)?;
                    element.limbs[i].copy_advice(|| "limb", &mut region, self.columns[0], 0)?;
                    low.copy_advice(|| "low word", &mut region, self.columns[1], 0)?;
                    high.copy_advice(|| "high word", &mut region, self.columns[2], 0)?;
                    Ok(())
                },
            )?;
            result.push(low);
            result.push(high);
        }
        result.reverse();
        result.try_into().map_err(|_| Error::Synthesis)
    }
    /// Pack up to 30 already range-constrained source byte cells, preserving
    /// their exact cell links. No unconstrained host digest/input is accepted.
    pub fn pack_bytes(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        bytes: &[Cell],
    ) -> Result<Element, Error> {
        assert!(!bytes.is_empty() && bytes.len() <= 30);
        let value = bytes
            .iter()
            .fold(Value::known(BigUint::from(0u32)), |acc, cell| {
                acc.zip(cell.value()).map(|(acc, byte)| {
                    acc * BigUint::from(256u32) + BigUint::from(byte.to_repr()[0])
                })
            });
        let element = self.assign(layouter, value)?;
        layouter.assign_region(
            || "source byte packing",
            |mut region| {
                for i in 0..4 {
                    self.pack.enable(&mut region, i)?;
                    element.limbs[i].copy_advice(
                        || "packed limb",
                        &mut region,
                        self.columns[0],
                        i,
                    )?;
                    for j in 0..8 {
                        let from_end = i * 8 + j;
                        if from_end < bytes.len() {
                            bytes[bytes.len() - 1 - from_end].copy_advice(
                                || "source byte",
                                &mut region,
                                self.columns[j + 1],
                                i,
                            )?;
                        } else {
                            region.assign_advice_from_constant(
                                || "zero padding",
                                self.columns[j + 1],
                                i,
                                pallas::Base::ZERO,
                            )?;
                        }
                    }
                }
                Ok(())
            },
        )?;
        Ok(element)
    }
    pub fn constant(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        value: BigUint,
    ) -> Result<Element, Error> {
        assert!(value < *MODULUS);
        let words = limb_values(&value);
        let cells = layouter.assign_region(
            || "constant BN254",
            |mut region| {
                let mut cells = Vec::new();
                for (i, word) in words.into_iter().enumerate() {
                    cells.push(region.assign_advice_from_constant(
                        || "constant limb",
                        self.columns[i],
                        0,
                        pallas::Base::from(word),
                    )?);
                }
                Ok(cells)
            },
        )?;
        Ok(Element {
            limbs: cells.try_into().map_err(|_| Error::Synthesis)?,
            value: Value::known(value),
        })
    }
    pub fn multiply(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        a: &Element,
        b: &Element,
    ) -> Result<Element, Error> {
        self.multiply_with_quotient(layouter, a, b, None)
    }
    // Private helper exposes a wrong quotient only to the in-module adversarial test.
    fn multiply_with_quotient(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        a: &Element,
        b: &Element,
        delta: Option<u64>,
    ) -> Result<Element, Error> {
        let product = a.value.clone().zip(b.value.clone()).map(|(a, b)| a * b);
        let quotient = self.assign(
            layouter,
            product
                .as_ref()
                .map(|p| p / &*MODULUS + BigUint::from(delta.unwrap_or(0))),
        )?;
        let remainder = self.assign(layouter, product.as_ref().map(|p| p % &*MODULUS))?;
        let vectors = a
            .value
            .as_ref()
            .zip(b.value.as_ref())
            .zip(quotient.value.as_ref())
            .zip(remainder.value.as_ref())
            .map(|(((a, b), q), r)| {
                (
                    limb_values(a),
                    limb_values(b),
                    limb_values(q),
                    limb_values(r),
                )
            });
        let modulus = limb_values(&MODULUS);
        let base = BigInt::from(1u128 << 64);
        let mut carry = Value::known(BigInt::from(0));
        let mut carry_cells = Vec::new();
        for k in 0..=8 {
            carry_cells.push(self.range(
                layouter,
                carry.clone().map(|v| v + BigInt::from(OFFSET)),
                72,
            )?);
            if k < 8 {
                carry = carry.zip(vectors).map(|(c, (a, b, q, r))| {
                    let mut sum = c;
                    for i in 0..4 {
                        for j in 0..4 {
                            if i + j == k {
                                sum += BigInt::from(a[i]) * BigInt::from(b[j])
                                    - BigInt::from(q[i]) * BigInt::from(modulus[j]);
                            }
                        }
                    }
                    if k < 4 {
                        sum -= BigInt::from(r[k]);
                    }
                    sum / &base
                });
            }
        }
        layouter.assign_region(
            || "integer product",
            |mut region| {
                region
                    .constrain_constant(carry_cells[0].cell(), pallas::Base::from_u128(OFFSET))?;
                region
                    .constrain_constant(carry_cells[8].cell(), pallas::Base::from_u128(OFFSET))?;
                for k in 0..8 {
                    self.multiplication[k].enable(&mut region, k)?;
                    for (i, cell) in a
                        .limbs
                        .iter()
                        .chain(&b.limbs)
                        .chain(&quotient.limbs)
                        .chain(&remainder.limbs)
                        .enumerate()
                    {
                        cell.copy_advice(|| "product input limb", &mut region, self.columns[i], k)?;
                    }
                    carry_cells[k].copy_advice(|| "carry", &mut region, self.columns[16], k)?;
                    carry_cells[k + 1].copy_advice(
                        || "next carry",
                        &mut region,
                        self.columns[17],
                        k,
                    )?;
                }
                Ok(())
            },
        )?;
        Ok(remainder)
    }
    pub fn add(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        a: &Element,
        b: &Element,
    ) -> Result<Element, Error> {
        let sum = a.value.clone().zip(b.value.clone()).map(|(a, b)| a + b);
        let remainder = self.assign(layouter, sum.as_ref().map(|v| v % &*MODULUS))?;
        let quotient = sum.as_ref().map(|v| (v / &*MODULUS).to_u64().unwrap());
        let q = layouter.assign_region(
            || "addition quotient",
            |mut region| {
                region.assign_advice(
                    || "quotient",
                    self.columns[0],
                    0,
                    || quotient.map(pallas::Base::from),
                )
            },
        )?;
        let vectors = a
            .value
            .as_ref()
            .zip(b.value.as_ref())
            .zip(remainder.value.as_ref())
            .map(|((a, b), r)| (limb_values(a), limb_values(b), limb_values(r)));
        let modulus = limb_values(&MODULUS);
        let mut carry = Value::known(0i128);
        let mut carries = Vec::new();
        for i in 0..=4 {
            carries.push(self.range(
                layouter,
                carry.map(|v| BigInt::from(v) + BigInt::from(OFFSET)),
                72,
            )?);
            if i < 4 {
                carry = carry.zip(vectors).zip(quotient).map(|((c, (a, b, r)), q)| {
                    (a[i] as i128 + b[i] as i128
                        - r[i] as i128
                        - (q as i128) * (modulus[i] as i128)
                        + c)
                        / (1i128 << 64)
                });
            }
        }
        layouter.assign_region(
            || "integer addition",
            |mut region| {
                region.constrain_constant(carries[0].cell(), pallas::Base::from_u128(OFFSET))?;
                region.constrain_constant(carries[4].cell(), pallas::Base::from_u128(OFFSET))?;
                for i in 0..4 {
                    self.addition[i].enable(&mut region, i)?;
                    for (col, cell) in [
                        self.columns[0],
                        self.columns[1],
                        self.columns[2],
                        self.columns[3],
                        self.columns[4],
                        self.columns[5],
                    ]
                    .into_iter()
                    .zip([
                        &a.limbs[i],
                        &b.limbs[i],
                        &remainder.limbs[i],
                        &q,
                        &carries[i],
                        &carries[i + 1],
                    ]) {
                        cell.copy_advice(|| "addition operand", &mut region, col, i)?;
                    }
                }
                Ok(())
            },
        )?;
        Ok(remainder)
    }
}

/// Small arithmetic gate used to measure/check the foundation before attempting
/// a complete Poseidon2/native binding circuit. Inputs and outputs are public
/// limbs in this experiment; it is not a privacy demonstration.
#[derive(Clone, Default)]
pub struct ArithmeticGate {
    pub a: Value<BigUint>,
    pub b: Value<BigUint>,
    #[cfg(test)]
    pub wrong_quotient: Option<u64>,
}
impl halo2_proofs::plonk::Circuit<pallas::Base> for ArithmeticGate {
    type Config = (Config, Column<halo2_proofs::plonk::Instance>);
    type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self::default()
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
        let a = config.assign(&mut layouter, self.a.clone())?;
        let b = config.assign(&mut layouter, self.b.clone())?;
        let sum = config.add(&mut layouter, &a, &b)?;
        #[cfg(test)]
        let product = config.multiply_with_quotient(&mut layouter, &a, &b, self.wrong_quotient)?;
        #[cfg(not(test))]
        let product = config.multiply(&mut layouter, &a, &b)?;
        for (i, cell) in a
            .limbs
            .iter()
            .chain(&b.limbs)
            .chain(&sum.limbs)
            .chain(&product.limbs)
            .enumerate()
        {
            layouter.constrain_instance(cell.cell(), instance, i)?;
        }
        Ok(())
    }
}
pub fn arithmetic_public(a: &BigUint, b: &BigUint) -> Vec<pallas::Base> {
    [
        a.clone(),
        b.clone(),
        (a + b) % &*MODULUS,
        (a * b) % &*MODULUS,
    ]
    .iter()
    .flat_map(limb_values)
    .map(pallas::Base::from)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::{BigInteger, PrimeField};
    use halo2_proofs::dev::MockProver;
    #[test]
    fn modulus_matches_pinned_bn254_and_rejects_aliases_and_wrong_quotients() {
        assert_eq!(
            *MODULUS,
            BigUint::from_bytes_le(&ark_bn254::Fr::MODULUS.to_bytes_le())
        );
        let a = &*MODULUS - BigUint::from(2u64);
        let b = &*MODULUS - BigUint::from(3u64);
        let circuit = ArithmeticGate {
            a: Value::known(a.clone()),
            b: Value::known(b.clone()),
            wrong_quotient: None,
        };
        let public = arithmetic_public(&a, &b);
        MockProver::run(10, &circuit, vec![public.clone()])
            .unwrap()
            .assert_satisfied();
        let wrong = ArithmeticGate {
            wrong_quotient: Some(1),
            ..circuit
        };
        assert!(
            MockProver::run(10, &wrong, vec![public])
                .unwrap()
                .verify()
                .is_err()
        );
        let a = &*MODULUS + BigUint::one();
        let b = BigUint::from(5u64);
        let wrong = ArithmeticGate {
            a: Value::known(a.clone()),
            b: Value::known(b.clone()),
            wrong_quotient: None,
        };
        assert!(
            MockProver::run(10, &wrong, vec![arithmetic_public(&a, &b)])
                .unwrap()
                .verify()
                .is_err()
        );
    }
    #[test]
    fn arithmetic_crosses_limbs_and_modulus_boundaries() {
        let cases = [
            BigUint::from(0u64),
            BigUint::one(),
            (BigUint::one() << 64) - BigUint::one(),
            BigUint::one() << 128,
            &*MODULUS - BigUint::one(),
        ];
        for a in &cases {
            for b in &cases {
                let circuit = ArithmeticGate {
                    a: Value::known(a.clone()),
                    b: Value::known(b.clone()),
                    wrong_quotient: None,
                };
                MockProver::run(10, &circuit, vec![arithmetic_public(a, b)])
                    .unwrap()
                    .assert_satisfied();
            }
        }
    }
}
