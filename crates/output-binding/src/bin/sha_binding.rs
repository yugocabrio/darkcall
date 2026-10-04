//! Shared SHA-256 row → native Ironwood cmx, including canonical point bytes.
//! Experimental/classical binding. ZIP 2005 rseed and ciphertext are not proved.
use darkcall_sha_link::{Cell, Config as ShaConfig};
use ff::Field;
use group::{Curve, GroupEncoding};
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
use pasta_curves::{pallas, vesta};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use std::time::Instant;

const K: u32 = 17;
const DOMAIN: &[u8; 16] = b"DarkCall/row/v1\0";
type SinConfig = SinsemillaConfig<OrchardHashDomains, OrchardCommitDomains, OrchardFixedBases>;
type SinChip = SinsemillaChip<OrchardHashDomains, OrchardCommitDomains, OrchardFixedBases>;
#[derive(Clone)]
struct Config {
    advice: [Column<halo2_proofs::plonk::Advice>; 10],
    instance: Column<Instance>,
    note: NoteCommitConfig,
    ecc: EccConfig<OrchardFixedBases>,
    sin: SinConfig,
    sha: ShaConfig,
}
#[derive(Clone, Default)]
struct Binding {
    gd: Value<pallas::Affine>,
    pkd: Value<pallas::Affine>,
    value: Value<u64>,
    rho: Value<pallas::Base>,
    psi: Value<pallas::Base>,
    rcm: Value<pallas::Scalar>,
    // conditions, accepted root, ticket, role, action index; all SHA big-endian words.
    prefix: [Value<u32>; 26],
    salt: [Value<u32>; 8],
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
        layouter.constrain_instance(action_index.cell(), config.instance, 9)?;
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
            layouter.constrain_instance(word.cell(), config.instance, i)?;
        }
        layouter.constrain_instance(cm.extract_p().inner().cell(), config.instance, 8)
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
    let rho = note.rho();
    let psi = note.rseed().psi(&rho);
    let rcm = note
        .rseed()
        .rcm_v3(&rho, &gd, &pkd, note.value().inner(), &psi)
        .inner();
    let mut row = [0u8; 224];
    row[..16].copy_from_slice(DOMAIN);
    rng.fill_bytes(&mut row[16..112]); // synthetic conditions/root/ticket
    row[112..116].copy_from_slice(&0u32.to_le_bytes()); // refund role
    row[116..120].copy_from_slice(&(action_index as u32).to_le_bytes());
    row[120..152].copy_from_slice(gd.to_bytes().as_ref());
    row[152..184].copy_from_slice(pkd.to_bytes().as_ref());
    row[184..192].copy_from_slice(&50u64.to_le_bytes());
    rng.fill_bytes(&mut row[192..224]);
    let digest = Sha256::digest(row);
    let circuit = Binding {
        gd: Value::known(gd.to_affine()),
        pkd: Value::known(pkd.to_affine()),
        value: Value::known(50),
        rho: Value::known(rho.into_inner()),
        psi: Value::known(psi),
        rcm: Value::known(rcm),
        prefix: std::array::from_fn(|i| {
            Value::known(u32::from_be_bytes(
                row[16 + i * 4..20 + i * 4].try_into().unwrap(),
            ))
        }),
        salt: std::array::from_fn(|i| {
            Value::known(u32::from_be_bytes(
                row[192 + i * 4..196 + i * 4].try_into().unwrap(),
            ))
        }),
    };
    let cmx = bundle.actions()[action_index].cmx().to_bytes();
    let mut public: Vec<_> = digest
        .chunks_exact(4)
        .map(|x| pallas::Base::from(u32::from_be_bytes(x.try_into().unwrap()) as u64))
        .collect();
    public.push(ExtractedNoteCommitment::from_bytes(&cmx).unwrap().inner());
    public.push(pallas::Base::from(action_index as u64));
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if args.len() != 2
            || args[0] != "--export-mpc-input"
            || args[1].is_empty()
            || !args[1]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("usage: sha_binding [--export-mpc-input LOCAL_FIXTURE_NAME]".into());
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let local = root.join(".local").canonicalize()?;
        let directory = local.join(&args[1]);
        std::fs::create_dir(&directory)?;
        std::fs::write(
            directory.join("Prover.toml"),
            format!("input = {}\n", serde_json::to_string(row.as_slice())?),
        )?;
        let digest_words: Vec<_> = digest
            .chunks_exact(4)
            .map(|x| u32::from_be_bytes(x.try_into().unwrap()))
            .collect();
        std::fs::write(
            directory.join("row-public.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"digest_words": digest_words, "action_index": action_index, "cmx": cmx}),
            )?,
        )?;
        eprintln!(
            "Synthetic MPC row input exported under ignored .local/{}",
            args[1]
        );
    }
    eprintln!("SHA/native mock started");
    MockProver::run(K, &circuit, vec![public.clone()])?.assert_satisfied();
    eprintln!("SHA/native mock passed; key generation started");
    let params: Params<vesta::Affine> = Params::new(K);
    let vk = keygen_vk(&params, &circuit.without_witnesses())?;
    let pk = keygen_pk(&params, vk, &circuit.without_witnesses())?;
    eprintln!("SHA/native proof generation started");
    let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(Vec::new());
    create_proof(
        &params,
        &pk,
        std::slice::from_ref(&circuit),
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
    for index in [0, 8, 9] {
        let mut changed = public.clone();
        changed[index] += pallas::Base::ONE;
        assert!(verify(&changed).is_err());
    }
    let wrong_amount = Binding {
        value: Value::known(51),
        ..circuit.clone()
    };
    assert!(
        MockProver::run(K, &wrong_amount, vec![public.clone()])?
            .verify()
            .is_err()
    );
    let wrong_recipient = Binding {
        gd: Value::known((-*gd).to_affine()),
        ..circuit
    };
    assert!(
        MockProver::run(K, &wrong_recipient, vec![public])?
            .verify()
            .is_err()
    );
    println!(
        "{}",
        serde_json::json!({
            "scope": "SHA-256 settlement row binds canonical recipient/amount to native Ironwood cmx",
            "k": K, "row_bytes": 224, "proof_bytes": proof.len(), "proof_verified": true,
            "canonical_point_encoding": true, "native_output_binding": true,
            "auction_plan_connected": false, "classical_binding_assumption": true,
            "rseed_derivation_constrained": false, "ciphertext_constrained": false,
            "changed_hash_cmx_index_rejected": true, "changed_amount_recipient_rejected": true,
            "seconds": start.elapsed().as_secs_f64(),
        })
    );
    Ok(())
}
