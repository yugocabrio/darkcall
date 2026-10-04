//! Reusable SHA/native circuit. Experimental/classical binding; rseed derivation
//! and ciphertext remain outside this additional proof.
use darkcall_sha_link::{Cell, Config as ShaConfig};
use halo2_gadgets::{
    ecc::{
        NonIdentityPoint, ScalarFixed,
        chip::{CircuitVersion, EccChip, EccConfig},
    },
    sinsemilla::chip::{SinsemillaChip, SinsemillaConfig},
    utilities::lookup_range_check::{LookupRangeCheck, LookupRangeCheckConfig},
};
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    plonk::{Circuit, Column, ConstraintSystem, Error, Instance},
};
use orchard::{
    circuit::note_commit::{NoteCommitChip, NoteCommitConfig, gadgets},
    constants::{OrchardCommitDomains, OrchardFixedBases, OrchardHashDomains},
    value::NoteValue,
};
use pasta_curves::pallas;

pub const DOMAIN: &[u8; 16] = b"DarkCall/row/v1\0";
type SinConfig = SinsemillaConfig<OrchardHashDomains, OrchardCommitDomains, OrchardFixedBases>;
type SinChip = SinsemillaChip<OrchardHashDomains, OrchardCommitDomains, OrchardFixedBases>;
#[derive(Clone)]
pub struct Config {
    advice: [Column<halo2_proofs::plonk::Advice>; 10],
    instance: Column<Instance>,
    note: NoteCommitConfig,
    ecc: EccConfig<OrchardFixedBases>,
    sin: SinConfig,
    sha: ShaConfig,
}
#[derive(Clone, Default)]
pub struct Binding {
    pub gd: Value<pallas::Affine>,
    pub pkd: Value<pallas::Affine>,
    pub value: Value<u64>,
    pub rho: Value<pallas::Base>,
    pub psi: Value<pallas::Base>,
    pub rcm: Value<pallas::Scalar>,
    // conditions, accepted root, ticket, role, action index; all SHA big-endian words.
    pub prefix: [Value<u32>; 26],
    pub salt: [Value<u32>; 8],
}
fn assign(
    layouter: &mut impl Layouter<pallas::Base>,
    column: Column<halo2_proofs::plonk::Advice>,
    value: Value<pallas::Base>,
) -> Result<Cell, Error> {
    layouter.assign_region(
        || "native field",
        |mut region| region.assign_advice(|| "field", column, 0, || value),
    )
}
impl Circuit<pallas::Base> for Binding {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self::default()
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Config {
        let advice = std::array::from_fn(|_| meta.advice_column());
        for col in advice {
            meta.enable_equality(col);
        }
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        let constant = meta.fixed_column();
        meta.enable_constant(constant);
        let idx = meta.lookup_table_column();
        let lookup = (idx, meta.lookup_table_column(), meta.lookup_table_column());
        let fixed = std::array::from_fn(|_| meta.fixed_column());
        let range = LookupRangeCheckConfig::configure(meta, advice[9], idx);
        let sin = SinChip::configure(
            meta,
            advice[..5].try_into().unwrap(),
            advice[2],
            fixed[0],
            lookup,
            range,
            false,
        );
        let note = NoteCommitChip::configure(meta, advice, sin.clone());
        let ecc = EccChip::<OrchardFixedBases>::configure(meta, advice, fixed, range);
        let sha = ShaConfig::configure(meta);
        Config {
            advice,
            instance,
            note,
            ecc,
            sin,
            sha,
        }
    }
    fn synthesize(
        &self,
        config: Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        SinChip::load(config.sin.clone(), &mut layouter)?;
        config.sha.load(&mut layouter)?;
        self.synthesize_row(&config, layouter, 0)
    }
}
impl Binding {
    fn synthesize_row(
        &self,
        config: &Config,
        mut layouter: impl Layouter<pallas::Base>,
        offset: usize,
    ) -> Result<(), Error> {
        let sin = SinChip::construct(config.sin.clone());
        let ecc = EccChip::construct(config.ecc.clone(), CircuitVersion::AnchoredBase);
        let note_chip = NoteCommitChip::construct(config.note.clone());
        let gd = NonIdentityPoint::new(ecc.clone(), layouter.namespace(|| "gd"), self.gd)?;
        let pkd = NonIdentityPoint::new(ecc.clone(), layouter.namespace(|| "pkd"), self.pkd)?;
        let value = layouter.assign_region(
            || "native value",
            |mut region| {
                region.assign_advice(
                    || "value",
                    config.advice[0],
                    0,
                    || self.value.map(NoteValue::from_raw),
                )
            },
        )?;
        let amount = assign(
            &mut layouter,
            config.advice[0],
            self.value.map(pallas::Base::from),
        )?;
        layouter.assign_region(
            || "bind native amount",
            |mut region| region.constrain_equal(value.cell(), amount.cell()),
        )?;
        let rho = assign(&mut layouter, config.advice[0], self.rho)?;
        let psi = assign(&mut layouter, config.advice[0], self.psi)?;
        let rcm = ScalarFixed::new(ecc.clone(), layouter.namespace(|| "rcm"), self.rcm)?;
        let cm = gadgets::note_commit(
            layouter.namespace(|| "native commitment"),
            sin,
            ecc,
            note_chip,
            gd.inner(),
            pkd.inner(),
            value,
            rho,
            psi,
            rcm,
        )?;
        let mut words = Vec::new();
        for bytes in DOMAIN.chunks_exact(4) {
            words.push(
                config
                    .sha
                    .constant(&mut layouter, u32::from_be_bytes(bytes.try_into().unwrap()))?,
            );
        }
        for value in self.prefix {
            words.push(config.sha.word(&mut layouter, value)?);
        }
        // The action index is encoded little-endian in the row and also exposed
        // for the verifier to select the actual transaction action's cmx.
        let action_bytes = config.sha.word_bytes(&mut layouter, &words[29])?;
        let action_index = config.sha.bytes_word(&mut layouter, &action_bytes, true)?;
        layouter.constrain_instance(action_index.cell(), config.instance, offset + 9)?;
        words.extend(config.sha.compressed_point(
            &mut layouter,
            &gd.inner().x(),
            &gd.inner().y(),
        )?);
        words.extend(config.sha.compressed_point(
            &mut layouter,
            &pkd.inner().x(),
            &pkd.inner().y(),
        )?);
        let low = config
            .sha
            .word(&mut layouter, self.value.map(|x| x as u32))?;
        let high = config
            .sha
            .word(&mut layouter, self.value.map(|x| (x >> 32) as u32))?;
        config
            .sha
            .bind_u64(&mut layouter, &amount, &[low.clone(), high.clone()])?;
        for word in [low, high] {
            let bytes = config.sha.word_bytes(&mut layouter, &word)?;
            words.push(config.sha.bytes_word(&mut layouter, &bytes, true)?);
        }
        for value in self.salt {
            words.push(config.sha.word(&mut layouter, value)?);
        }
        let digest = config.sha.hash_224(
            &mut layouter,
            &words.try_into().map_err(|_| Error::Synthesis)?,
        )?;
        for (i, word) in digest.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.instance, offset + i)?;
        }
        layouter.constrain_instance(cm.extract_p().inner().cell(), config.instance, offset + 8)
    }
}

/// A fixed-count native proof binds every row. Canonical row ordering and the
/// action permutation are cross-checked against the auction's public statement.
#[derive(Clone)]
pub struct Batch<const N: usize>(pub [Binding; N]);
impl<const N: usize> Circuit<pallas::Base> for Batch<N> {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self(std::array::from_fn(|_| Binding::default()))
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Config {
        Binding::configure(meta)
    }
    fn synthesize(
        &self,
        config: Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        SinChip::load(config.sin.clone(), &mut layouter)?;
        config.sha.load(&mut layouter)?;
        for (i, row) in self.0.iter().enumerate() {
            row.synthesize_row(
                &config,
                layouter.namespace(|| format!("native row {i}")),
                i * 10,
            )?;
        }
        Ok(())
    }
}
