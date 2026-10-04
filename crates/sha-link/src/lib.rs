//! Experimental SHA-256 links for the auction/native commitment bridge.
//! Uses the vendored Table16 extension. Standard padding, fixed IV, block
//! chaining and modulo-2^32 feed-forward are constrained here, not on the host.
use ff::{Field, PrimeField};
use halo2_gadgets::sha256::{Table16Chip, Table16Config};
use halo2_proofs::{
    circuit::{AssignedCell, Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, Selector, TableColumn},
    poly::Rotation,
};
use pasta_curves::pallas;

pub type Cell = AssignedCell<pallas::Base, pallas::Base>;
const IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

#[derive(Clone, Debug)]
pub struct Config {
    sha: Option<Table16Config>,
    columns: [Column<Advice>; 5],
    range: Selector,
    add: Selector,
    borrow: Selector,
    compose: Selector,
    bit: Selector,
    pack: Selector,
    table: TableColumn,
}

impl Config {
    pub fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self {
        Self::configure_inner(meta, true)
    }
    /// Canonical byte/point encoding without unused SHA round/lookup columns.
    /// Used only by the alternative hash circuit; SHA's configured shape stays.
    pub fn configure_encoding(meta: &mut ConstraintSystem<pallas::Base>) -> Self {
        Self::configure_inner(meta, false)
    }
    fn configure_inner(meta: &mut ConstraintSystem<pallas::Base>, with_sha: bool) -> Self {
        let sha = with_sha.then(|| Table16Chip::configure(meta));
        let columns = std::array::from_fn(|_| meta.advice_column());
        for col in columns {
            meta.enable_equality(col);
        }
        let constant = meta.fixed_column();
        meta.enable_constant(constant);
        let range = meta.complex_selector();
        let add = meta.selector();
        let borrow = meta.selector();
        let compose = meta.selector();
        let bit = meta.selector();
        let pack = meta.selector();
        let table = meta.lookup_table_column();
        for col in columns.iter().skip(1) {
            meta.lookup(|meta| {
                vec![(
                    meta.query_selector(range) * meta.query_advice(*col, Rotation::cur()),
                    table,
                )]
            });
        }
        meta.create_gate("u32 byte decomposition", |meta| {
            let q = meta.query_selector(range);
            let word = meta.query_advice(columns[0], Rotation::cur());
            let bytes = (0..4).map(|i| {
                meta.query_advice(columns[i + 1], Rotation::cur())
                    * pallas::Base::from(1u64 << (8 * i))
            });
            vec![q * (word - bytes.reduce(|a, b| a + b).unwrap())]
        });
        meta.create_gate("SHA feed-forward modulo 2^32", |meta| {
            let q = meta.query_selector(add);
            let old = meta.query_advice(columns[0], Rotation::cur());
            let raw = meta.query_advice(columns[1], Rotation::cur());
            let sum = meta.query_advice(columns[2], Rotation::cur());
            let carry = meta.query_advice(columns[3], Rotation::cur());
            vec![
                q.clone() * (old + raw - sum - carry.clone() * pallas::Base::from(1u64 << 32)),
                q * carry.clone() * (carry - Expression::Constant(pallas::Base::from(1))),
            ]
        });
        meta.create_gate("canonical field subtraction", |meta| {
            let q = meta.query_selector(borrow);
            let x = meta.query_advice(columns[0], Rotation::cur());
            let bound = meta.query_advice(columns[1], Rotation::cur());
            let difference = meta.query_advice(columns[2], Rotation::cur());
            let previous = meta.query_advice(columns[3], Rotation::cur());
            let next = meta.query_advice(columns[4], Rotation::cur());
            vec![
                q.clone()
                    * (bound - x - previous + next.clone() * pallas::Base::from(1u64 << 32)
                        - difference),
                q * next.clone() * (next - Expression::Constant(pallas::Base::ONE)),
            ]
        });
        meta.create_gate("field limb accumulation", |meta| {
            let q = meta.query_selector(compose);
            let previous = meta.query_advice(columns[0], Rotation::cur());
            let word = meta.query_advice(columns[1], Rotation::cur());
            let next = meta.query_advice(columns[2], Rotation::cur());
            vec![q * (previous * pallas::Base::from(1u64 << 32) + word - next)]
        });
        meta.create_gate("u32 bit decomposition", |meta| {
            let q = meta.query_selector(bit);
            let previous = meta.query_advice(columns[0], Rotation::cur());
            let bit = meta.query_advice(columns[1], Rotation::cur());
            let next = meta.query_advice(columns[2], Rotation::cur());
            vec![
                q.clone() * (previous * pallas::Base::from(2) + bit.clone() - next),
                q * bit.clone() * (bit - Expression::Constant(pallas::Base::ONE)),
            ]
        });
        meta.create_gate("compressed point high word", |meta| {
            let q = meta.query_selector(pack);
            let x = meta.query_advice(columns[0], Rotation::cur());
            let parity = meta.query_advice(columns[1], Rotation::cur());
            let compressed = meta.query_advice(columns[2], Rotation::cur());
            vec![q * (x + parity * pallas::Base::from(1u64 << 31) - compressed)]
        });
        Self {
            sha,
            columns,
            range,
            add,
            borrow,
            compose,
            bit,
            pack,
            table,
        }
    }

    pub fn load(&self, layouter: &mut impl Layouter<pallas::Base>) -> Result<(), Error> {
        if let Some(sha) = &self.sha {
            Table16Chip::load(sha.clone(), layouter)?;
        }
        self.load_bytes(layouter)
    }

    pub fn load_bytes(&self, layouter: &mut impl Layouter<pallas::Base>) -> Result<(), Error> {
        layouter.assign_table(
            || "bytes",
            |mut table| {
                for i in 0..256 {
                    table.assign_cell(
                        || "u8",
                        self.table,
                        i,
                        || Value::known(pallas::Base::from(i as u64)),
                    )?;
                }
                Ok(())
            },
        )
    }

    pub fn word(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        value: Value<u32>,
    ) -> Result<Cell, Error> {
        layouter.assign_region(
            || "source word",
            |mut region| {
                self.range.enable(&mut region, 0)?;
                let cell = region.assign_advice(
                    || "source word",
                    self.columns[0],
                    0,
                    || value.map(|x| pallas::Base::from(x as u64)),
                )?;
                for i in 0..4 {
                    region.assign_advice(
                        || "word byte",
                        self.columns[i + 1],
                        0,
                        || value.map(|x| pallas::Base::from(((x >> (8 * i)) & 255) as u64)),
                    )?;
                }
                Ok(cell)
            },
        )
    }

    pub fn constant(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        value: u32,
    ) -> Result<Cell, Error> {
        layouter.assign_region(
            || "SHA fixed word",
            |mut region| {
                region.assign_advice_from_constant(
                    || "fixed word",
                    self.columns[0],
                    0,
                    pallas::Base::from(value as u64),
                )
            },
        )
    }

    /// Decompose an existing word, constraining both its u32 range and its bytes.
    /// Returns the four bytes in little-endian order.
    pub fn word_bytes(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        word: &Cell,
    ) -> Result<[Cell; 4], Error> {
        let value = word
            .value()
            .map(|x| u32::from_le_bytes(x.to_repr()[..4].try_into().unwrap()));
        layouter.assign_region(
            || "linked word bytes",
            |mut region| {
                self.range.enable(&mut region, 0)?;
                word.copy_advice(|| "word", &mut region, self.columns[0], 0)?;
                let mut bytes = Vec::new();
                for i in 0..4 {
                    bytes.push(region.assign_advice(
                        || "byte",
                        self.columns[i + 1],
                        0,
                        || value.map(|x| pallas::Base::from(((x >> (8 * i)) & 255) as u64)),
                    )?);
                }
                Ok(bytes.try_into().unwrap())
            },
        )
    }

    /// Encode four existing byte cells as a word, with range checks and copies.
    pub fn bytes_word(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        bytes: &[Cell; 4],
        big_endian: bool,
    ) -> Result<Cell, Error> {
        let order = if big_endian {
            [3, 2, 1, 0]
        } else {
            [0, 1, 2, 3]
        };
        let value =
            order
                .iter()
                .enumerate()
                .fold(Value::known(pallas::Base::ZERO), |sum, (i, j)| {
                    sum.zip(bytes[*j].value())
                        .map(|(a, b)| a + *b * pallas::Base::from(1u64 << (i * 8)))
                });
        layouter.assign_region(
            || "word from linked bytes",
            |mut region| {
                self.range.enable(&mut region, 0)?;
                let word = region.assign_advice(|| "encoded word", self.columns[0], 0, || value)?;
                for i in 0..4 {
                    bytes[order[i]].copy_advice(
                        || "linked byte",
                        &mut region,
                        self.columns[i + 1],
                        0,
                    )?;
                }
                Ok(word)
            },
        )
    }

    /// Enforce eight little-endian u32 words encode the canonical Pallas field
    /// element, not an alternative integer that differs by the field modulus.
    pub fn assert_canonical_field(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        field: &Cell,
        words: &[Cell; 8],
    ) -> Result<(), Error> {
        let bound = (-pallas::Base::ONE).to_repr();
        let mut previous = self.constant(layouter, 0)?;
        for i in 0..8 {
            // This range check is necessary even when supplied by another gadget.
            self.word_bytes(layouter, &words[i])?;
            let max_word = u32::from_le_bytes(bound[i * 4..i * 4 + 4].try_into().unwrap());
            let max_cell = self.constant(layouter, max_word)?;
            let result = words[i].value().zip(previous.value()).map(|(x, b)| {
                let xi = u32::from_le_bytes(x.to_repr()[..4].try_into().unwrap()) as i64;
                let bi = u32::from_le_bytes(b.to_repr()[..4].try_into().unwrap()) as i64;
                let difference = max_word as i64 - xi - bi;
                if difference < 0 {
                    ((difference + (1i64 << 32)) as u32, 1u64)
                } else {
                    (difference as u32, 0)
                }
            });
            let difference = self.word(layouter, result.map(|x| x.0))?;
            previous = layouter.assign_region(
                || "canonical subtract",
                |mut region| {
                    self.borrow.enable(&mut region, 0)?;
                    words[i].copy_advice(|| "limb", &mut region, self.columns[0], 0)?;
                    max_cell.copy_advice(
                        || "modulus minus one limb",
                        &mut region,
                        self.columns[1],
                        0,
                    )?;
                    difference.copy_advice(
                        || "bounded difference",
                        &mut region,
                        self.columns[2],
                        0,
                    )?;
                    previous.copy_advice(|| "incoming borrow", &mut region, self.columns[3], 0)?;
                    region.assign_advice(
                        || "outgoing borrow",
                        self.columns[4],
                        0,
                        || result.map(|x| pallas::Base::from(x.1)),
                    )
                },
            )?;
        }
        let zero = self.constant(layouter, 0)?;
        layouter.assign_region(
            || "canonical upper bound",
            |mut region| region.constrain_equal(previous.cell(), zero.cell()),
        )?;
        let mut accumulator = zero;
        for word in words.iter().rev() {
            let next_value = accumulator
                .value()
                .zip(word.value())
                .map(|(a, b)| *a * pallas::Base::from(1u64 << 32) + b);
            accumulator = layouter.assign_region(
                || "field accumulation",
                |mut region| {
                    self.compose.enable(&mut region, 0)?;
                    accumulator.copy_advice(|| "previous", &mut region, self.columns[0], 0)?;
                    word.copy_advice(|| "limb", &mut region, self.columns[1], 0)?;
                    region.assign_advice(|| "next", self.columns[2], 0, || next_value)
                },
            )?;
        }
        layouter.assign_region(
            || "bind canonical representation",
            |mut region| region.constrain_equal(accumulator.cell(), field.cell()),
        )
    }

    pub fn field_words(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        field: &Cell,
    ) -> Result<[Cell; 8], Error> {
        let bytes = field.value().map(|x| x.to_repr());
        let mut words = Vec::new();
        for i in 0..8 {
            words.push(self.word(
                layouter,
                bytes.map(|x| u32::from_le_bytes(x[i * 4..i * 4 + 4].try_into().unwrap())),
            )?);
        }
        let words: [Cell; 8] = words.try_into().unwrap();
        self.assert_canonical_field(layouter, field, &words)?;
        Ok(words)
    }

    /// Bind two range-checked little-endian u32 limbs to a native value cell.
    pub fn bind_u64(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        value: &Cell,
        words: &[Cell; 2],
    ) -> Result<(), Error> {
        for word in words {
            self.word_bytes(layouter, word)?;
        }
        layouter.assign_region(
            || "native u64 from limbs",
            |mut region| {
                self.compose.enable(&mut region, 0)?;
                words[1].copy_advice(|| "high limb", &mut region, self.columns[0], 0)?;
                words[0].copy_advice(|| "low limb", &mut region, self.columns[1], 0)?;
                value.copy_advice(|| "native amount", &mut region, self.columns[2], 0)?;
                Ok(())
            },
        )
    }

    fn parity(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        word: &Cell,
    ) -> Result<Cell, Error> {
        let value = word
            .value()
            .map(|x| u32::from_le_bytes(x.to_repr()[..4].try_into().unwrap()));
        let mut previous = self.constant(layouter, 0)?;
        let mut parity = None;
        for i in (0..32).rev() {
            let bit_value = value.map(|x| pallas::Base::from(((x >> i) & 1) as u64));
            let next_value = previous
                .value()
                .zip(bit_value)
                .map(|(a, b)| *a * pallas::Base::from(2) + b);
            let (next, bit) = layouter.assign_region(
                || "canonical y parity",
                |mut region| {
                    self.bit.enable(&mut region, 0)?;
                    previous.copy_advice(|| "previous", &mut region, self.columns[0], 0)?;
                    let bit = region.assign_advice(|| "bit", self.columns[1], 0, || bit_value)?;
                    let next =
                        region.assign_advice(|| "next", self.columns[2], 0, || next_value)?;
                    Ok((next, bit))
                },
            )?;
            previous = next;
            parity = Some(bit);
        }
        layouter.assign_region(
            || "bind y word parity",
            |mut region| region.constrain_equal(previous.cell(), word.cell()),
        )?;
        Ok(parity.unwrap())
    }

    /// Canonical compressed Pallas point bytes, returned as eight SHA big-endian
    /// words. The caller must provide coordinates from the constrained ECC point.
    /// This does not itself assert the point is on the curve or nonidentity.
    pub fn compressed_point(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        x: &Cell,
        y: &Cell,
    ) -> Result<[Cell; 8], Error> {
        let mut x_words = self.field_words(layouter, x)?;
        let y_words = self.field_words(layouter, y)?;
        let parity = self.parity(layouter, &y_words[0])?;
        let value = x_words[7]
            .value()
            .zip(parity.value())
            .map(|(a, b)| *a + *b * pallas::Base::from(1u64 << 31));
        x_words[7] = layouter.assign_region(
            || "compressed high word",
            |mut region| {
                self.pack.enable(&mut region, 0)?;
                x_words[7].copy_advice(|| "x high limb", &mut region, self.columns[0], 0)?;
                parity.copy_advice(|| "y parity", &mut region, self.columns[1], 0)?;
                region.assign_advice(|| "compressed limb", self.columns[2], 0, || value)
            },
        )?;
        let mut encoded = Vec::new();
        for word in x_words {
            let bytes = self.word_bytes(layouter, &word)?;
            encoded.push(self.bytes_word(layouter, &bytes, true)?);
        }
        Ok(encoded.try_into().unwrap())
    }

    fn add_words(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        old: &Cell,
        raw: &Cell,
    ) -> Result<Cell, Error> {
        let total = old.value().zip(raw.value()).map(|(a, b)| {
            u32::from_le_bytes(a.to_repr()[..4].try_into().unwrap()) as u64
                + u32::from_le_bytes(b.to_repr()[..4].try_into().unwrap()) as u64
        });
        let sum = self.word(layouter, total.map(|x| x as u32))?;
        layouter.assign_region(
            || "feed-forward",
            |mut region| {
                self.add.enable(&mut region, 0)?;
                old.copy_advice(|| "previous state", &mut region, self.columns[0], 0)?;
                raw.copy_advice(|| "raw round result", &mut region, self.columns[1], 0)?;
                sum.copy_advice(|| "feed-forward sum", &mut region, self.columns[2], 0)?;
                region.assign_advice(
                    || "carry",
                    self.columns[3],
                    0,
                    || total.map(|x| pallas::Base::from(x >> 32)),
                )?;
                Ok(())
            },
        )?;
        Ok(sum)
    }

    /// Hash exactly 224 private bytes supplied as 56 big-endian u32 words.
    /// The caller must use these same cells for the encoded settlement fields.
    pub fn hash_224(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        words: &[Cell; 56],
    ) -> Result<[Cell; 8], Error> {
        let mut padded = words.to_vec();
        padded.push(self.constant(layouter, 0x80000000)?);
        for _ in 57..63 {
            padded.push(self.constant(layouter, 0)?);
        }
        padded.push(self.constant(layouter, 224 * 8)?);
        self.hash_padded(layouter, &padded)
    }

    fn hash_padded(
        &self,
        layouter: &mut impl Layouter<pallas::Base>,
        padded: &[Cell],
    ) -> Result<[Cell; 8], Error> {
        assert!(!padded.is_empty() && padded.len().is_multiple_of(16));
        let mut state = Vec::new();
        for word in IV {
            state.push(self.constant(layouter, word)?);
        }
        let mut state: [Cell; 8] = state.try_into().unwrap();
        let chip = Table16Chip::construct(self.sha.clone().ok_or(Error::Synthesis)?);
        for block in padded.chunks_exact(16) {
            let initialized = chip.initialize_linked(layouter, &state)?;
            let raw_state =
                chip.compress_linked(layouter, &initialized, block.try_into().unwrap())?;
            let raw = chip.digest_linked(layouter, &raw_state)?;
            let mut next = Vec::new();
            for i in 0..8 {
                next.push(self.add_words(layouter, &state[i], &raw[i])?);
            }
            state = next.try_into().unwrap();
        }
        Ok(state)
    }
}
