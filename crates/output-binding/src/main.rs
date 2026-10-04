//! Smallest native commitment binding experiment. This Pallas Poseidon row
//! commitment is experimental and is NOT yet the auction circuit's plan hash.
//! The additional proof assumes classical commitment binding. It does not
//! constrain ZIP 2005's Blake2b rseed derivation or ciphertext correctness.
use ff::Field;
use group::Curve;
use halo2_gadgets::{
    ecc::{
        NonIdentityPoint, ScalarFixed,
        chip::{CircuitVersion, EccChip, EccConfig},
    },
    poseidon::{
        Hash, Pow5Chip, Pow5Config,
        primitives::{self, ConstantLength, P128Pow5T3},
    },
    sinsemilla::chip::{SinsemillaChip, SinsemillaConfig},
    utilities::lookup_range_check::{LookupRangeCheck, LookupRangeCheckConfig},
};
use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        Circuit, Column, ConstraintSystem, Error, Instance, SingleVerifier, create_proof,
        keygen_pk, keygen_vk, verify_proof,
    },
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite, Challenge255},
};
use orchard::{
    builder::{Builder, BundleType},
    bundle::{BundleVersion, Flags},
    circuit::note_commit::{NoteCommitChip, NoteCommitConfig, gadgets},
    constants::{OrchardCommitDomains, OrchardFixedBases, OrchardHashDomains},
    keys::{FullViewingKey, Scope, SpendingKey},
    note::ExtractedNoteCommitment,
    value::NoteValue,
};
use pasta_curves::{arithmetic::CurveAffine, pallas, vesta};
use rand::{RngCore, rngs::OsRng};
use std::time::Instant;

const LEN: usize = 21;
const K: u32 = 12;
type SinConfig = SinsemillaConfig<OrchardHashDomains, OrchardCommitDomains, OrchardFixedBases>;
type SinChip = SinsemillaChip<OrchardHashDomains, OrchardCommitDomains, OrchardFixedBases>;

#[derive(Clone)]
struct Config {
    advice: [Column<halo2_proofs::plonk::Advice>; 10],
    instance: Column<Instance>,
    note: NoteCommitConfig,
    ecc: EccConfig<OrchardFixedBases>,
    sin: SinConfig,
    poseidon: Pow5Config<pallas::Base, 3, 2>,
}

#[derive(Clone, Default)]
struct Binding {
    gd: Value<pallas::Affine>,
    pkd: Value<pallas::Affine>,
    value: Value<u64>,
    rho: Value<pallas::Base>,
    psi: Value<pallas::Base>,
    rcm: Value<pallas::Scalar>,
    // domain, conditions (4 limbs), accepted root (4), ticket (4), role, action.
    prefix: [Value<pallas::Base>; 15],
    salt: Value<pallas::Base>,
}

fn assign(
    layouter: &mut impl Layouter<pallas::Base>,
    column: Column<halo2_proofs::plonk::Advice>,
    value: Value<pallas::Base>,
) -> Result<AssignedCell<pallas::Base, pallas::Base>, Error> {
    layouter.assign_region(
        || "row field",
        |mut region| region.assign_advice(|| "value", column, 0, || value),
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
        for column in advice {
            meta.enable_equality(column);
        }
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        let constants = meta.fixed_column();
        meta.enable_constant(constants);
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
        let rc_a = std::array::from_fn(|_| meta.fixed_column());
        let rc_b = std::array::from_fn(|_| meta.fixed_column());
        let poseidon = Pow5Chip::configure::<P128Pow5T3>(
            meta,
            advice[..3].try_into().unwrap(),
            advice[3],
            rc_a,
            rc_b,
        );
        Config {
            advice,
            instance,
            note,
            ecc,
            sin,
            poseidon,
        }
    }
    fn synthesize(
        &self,
        config: Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        SinChip::load(config.sin.clone(), &mut layouter)?;
        let sin = SinChip::construct(config.sin);
        let ecc = EccChip::construct(config.ecc, CircuitVersion::AnchoredBase);
        let note_chip = NoteCommitChip::construct(config.note);
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
            || "bind amount to native value",
            |mut region| region.constrain_equal(value.cell(), amount.cell()),
        )?;
        let rho = assign(&mut layouter, config.advice[0], self.rho)?;
        let psi = assign(&mut layouter, config.advice[0], self.psi)?;
        let rcm = ScalarFixed::new(ecc.clone(), layouter.namespace(|| "rcm"), self.rcm)?;
        let cm = gadgets::note_commit(
            layouter.namespace(|| "native Sinsemilla commitment"),
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
        // Commit to the same constrained point coordinates and value used by NoteCommit.
        let mut fields = Vec::new();
        fields.push(layouter.assign_region(
            || "fixed row domain",
            |mut region| {
                region.assign_advice_from_constant(
                    || "DCROW",
                    config.advice[0],
                    0,
                    pallas::Base::from(0x4443524f57_u64),
                )
            },
        )?);
        for x in self.prefix.iter().skip(1) {
            fields.push(assign(&mut layouter, config.advice[0], *x)?);
        }
        fields.extend([
            gd.inner().x(),
            gd.inner().y(),
            pkd.inner().x(),
            pkd.inner().y(),
            amount,
        ]);
        fields.push(assign(&mut layouter, config.advice[0], self.salt)?);
        let row = Hash::<_, _, P128Pow5T3, ConstantLength<LEN>, 3, 2>::init(
            Pow5Chip::construct(config.poseidon),
            layouter.namespace(|| "row hash init"),
        )?
        .hash(
            layouter.namespace(|| "row hash"),
            fields.try_into().map_err(|_| Error::Synthesis)?,
        )?;
        layouter.constrain_instance(row.cell(), config.instance, 0)?;
        layouter.constrain_instance(cm.extract_p().inner().cell(), config.instance, 1)?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let mut rng = OsRng;
    let key = loop {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        if let Some(key) = SpendingKey::from_bytes(bytes).into_option() {
            break key;
        }
    };
    let fvk = FullViewingKey::from(&key);
    let address = fvk.address_at(0u32, Scope::External);
    let mut builder = Builder::new(
        BundleType::DEFAULT,
        BundleVersion::ironwood_v3(),
        Flags::SPENDS_DISABLED,
        orchard::Anchor::empty_tree(),
    )
    .unwrap();
    builder
        .add_output(None, address, NoteValue::from_raw(50), [0; 512])
        .unwrap();
    let (bundle, meta) = builder.build::<i64>(&mut rng).unwrap().unwrap();
    let action_index = meta.output_action_index(0).unwrap();
    let (note, recipient, _) = bundle
        .decrypt_output_with_key(action_index, &fvk.to_ivk(Scope::External))
        .unwrap();
    assert_eq!(recipient, address);
    let gd = address.g_d();
    let pkd = address.pk_d().inner();
    let gd_affine = gd.to_affine();
    let pkd_affine = pkd.to_affine();
    let rho = note.rho();
    let psi = note.rseed().psi(&rho);
    let rcm = note
        .rseed()
        .rcm_v3(&rho, &gd, &pkd, note.value().inner(), &psi)
        .inner();
    let salt = pallas::Base::random(&mut rng);
    let mut prefix = [pallas::Base::from(0); 15];
    // Synthetic four-limb condition/root/ticket encodings and row role/index.
    prefix[0] = pallas::Base::from(0x4443524f57_u64); // DCROW domain for prototype only.
    prefix[1] = pallas::Base::from(1);
    prefix[5] = pallas::Base::from(2);
    prefix[9] = pallas::Base::from(3);
    prefix[13] = pallas::Base::from(0);
    prefix[14] = pallas::Base::from(action_index as u64);
    let gd_coords = gd_affine.coordinates().unwrap();
    let pkd_coords = pkd_affine.coordinates().unwrap();
    let mut fields = prefix.to_vec();
    fields.extend([
        *gd_coords.x(),
        *gd_coords.y(),
        *pkd_coords.x(),
        *pkd_coords.y(),
        pallas::Base::from(50),
        salt,
    ]);
    let row = primitives::Hash::<_, P128Pow5T3, ConstantLength<LEN>, 3, 2>::init()
        .hash(fields.try_into().unwrap());
    let cmx: ExtractedNoteCommitment = note.commitment().into();
    assert_eq!(&cmx, bundle.actions()[action_index].cmx());
    let public = vec![row, cmx.inner()];
    let circuit = Binding {
        gd: Value::known(gd_affine),
        pkd: Value::known(pkd_affine),
        value: Value::known(50),
        rho: Value::known(rho.into_inner()),
        psi: Value::known(psi),
        rcm: Value::known(rcm),
        prefix: prefix.map(Value::known),
        salt: Value::known(salt),
    };
    MockProver::run(K, &circuit, vec![public.clone()])?.assert_satisfied();
    let mock_seconds = start.elapsed().as_secs_f64();
    eprintln!("native row constraints passed; generating independent Halo2 proof");
    let params: Params<vesta::Affine> = Params::new(K);
    let vk = keygen_vk(&params, &circuit)?;
    let pk = keygen_pk(&params, vk, &circuit)?;
    let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(Vec::new());
    create_proof(
        &params,
        &pk,
        std::slice::from_ref(&circuit),
        &[&[&public]],
        &mut rng,
        &mut transcript,
    )?;
    let proof = transcript.finalize();
    let verify = |inputs: &[pallas::Base]| {
        let mut reader = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(&proof[..]);
        verify_proof(
            &params,
            pk.get_vk(),
            SingleVerifier::new(&params),
            &[&[inputs]],
            &mut reader,
        )
    };
    verify(&public)?;
    let mut bad_row = public.clone();
    bad_row[0] += pallas::Base::from(1);
    assert!(verify(&bad_row).is_err());
    let mut bad_cmx = public.clone();
    bad_cmx[1] += pallas::Base::from(1);
    assert!(verify(&bad_cmx).is_err());
    // A changed private amount cannot use the original row/native commitment.
    let mut bad_amount = circuit.clone();
    bad_amount.value = Value::known(51);
    assert!(
        MockProver::run(K, &bad_amount, vec![public.clone()])?
            .verify()
            .is_err()
    );
    let mut bad_receiver = circuit;
    bad_receiver.pkd = Value::known(gd_affine);
    assert!(
        MockProver::run(K, &bad_receiver, vec![public])?
            .verify()
            .is_err()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "scope":"one private row to actual native Ironwood output commitment",
            "experimental_plan_hash":"Pallas Poseidon P128Pow5T3, length 21",
            "auction_plan_hash_connected":false,"ciphertext_proved":false,"zip2005_rseed_derivation_constrained":false,
            "binding_assumption":"classical commitment binding","proof_verified":true,
            "changed_amount_rejected":true,"changed_receiver_rejected":true,"changed_row_hash_rejected":true,"changed_native_commitment_rejected":true,
            "k":K,"proof_bytes":proof.len(),"mock_seconds":mock_seconds,"total_seconds":start.elapsed().as_secs_f64(),
        }))?
    );
    Ok(())
}
