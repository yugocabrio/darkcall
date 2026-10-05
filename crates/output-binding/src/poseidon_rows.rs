//! Fixed six-row Poseidon/native proof set. This verifies application statements,
//! not chain inclusion, custody authorization, rseed derivation or ciphertext.
use crate::sha::{Binding, PoseidonBinding, SharedPoseidonBinding};
use darkcall_settlement_types::{NativeBatchStatement, Row, link_four_bid_statements};
use group::{Curve, GroupEncoding};
use halo2_proofs::{
    circuit::Value,
    plonk::{
        Circuit, ConstraintSystem, ProvingKey, SingleVerifier, create_proof, keygen_pk, keygen_vk,
        verify_proof,
    },
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite, Challenge255},
};
use orchard::{
    Note,
    note::{ExtractedNoteCommitment, NoteVersion},
};
use pasta_curves::{pallas, vesta};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

pub const K: u32 = 17;
pub const HASH_RULE: &str = "bn254-poseidon2-packed-row-v1";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Public static circuit metadata only; no witness, SRS, key or proof allocation.
pub fn circuit_profile() -> Result<serde_json::Value> {
    profile::<PoseidonBinding>("working-33-column-v1")
}

pub const SHARED_LAYOUT: &str = "experimental-shared18-native-row-v1";
pub const WORKING_LAYOUT: &str = "working-33-column-v1";

/// Dispatches only two compiled circuits. Never imports a bundle-supplied VK.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowLayout {
    Working33,
    Shared18,
}
impl RowLayout {
    pub fn id(self) -> &'static str {
        match self {
            Self::Working33 => WORKING_LAYOUT,
            Self::Shared18 => SHARED_LAYOUT,
        }
    }
    pub fn from_id(id: &str) -> Result<Self> {
        match id {
            WORKING_LAYOUT => Ok(Self::Working33),
            SHARED_LAYOUT => Ok(Self::Shared18),
            _ => Err("unknown native binding layout".into()),
        }
    }
}

/// Missing metadata denotes the original historical circuit only. Present but
/// invalid metadata fails closed; it never falls back to another circuit.
pub fn read_layout(directory: &std::path::Path) -> Result<RowLayout> {
    use std::io::Read;
    let path = directory.join("native-layout.json");
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(value) => value,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(RowLayout::Working33),
        Err(e) => return Err(e.into()),
    };
    if !metadata.is_file() || metadata.len() > 4096 {
        return Err("bounded regular native layout metadata required".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err("native layout metadata exceeds bound".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let object = value.as_object().ok_or("native layout object required")?;
    if object.len() != 2 || value["format"] != 1 {
        return Err("native layout metadata format/fields".into());
    }
    RowLayout::from_id(
        value["layout"]
            .as_str()
            .ok_or("native layout id required")?,
    )
}

pub fn write_layout(directory: &std::path::Path, layout: RowLayout) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(directory.join("native-layout.json"))?;
    file.write_all(&serde_json::to_vec(
        &serde_json::json!({"format":1,"layout":layout.id()}),
    )?)?;
    file.sync_all()?;
    std::fs::File::open(directory)?.sync_all()?;
    Ok(())
}

pub fn shared_circuit_profile() -> Result<serde_json::Value> {
    profile::<SharedPoseidonBinding>(SHARED_LAYOUT)
}

/// Constraints only. No parameters, trusted key or SNARK proof is generated.
pub fn shared_mock_satisfied(binding: Binding, public: Vec<pallas::Base>) -> Result<bool> {
    let prover =
        halo2_proofs::dev::MockProver::run(K, &SharedPoseidonBinding(binding), vec![public])?;
    Ok(prover.verify().is_ok())
}

fn profile<C: Circuit<pallas::Base>>(layout: &str) -> Result<serde_json::Value> {
    let mut meta = ConstraintSystem::<pallas::Base>::default();
    C::configure(&mut meta);
    let description = format!("{:?}", meta.pinned());
    let count = |name: &str| -> Result<usize> {
        let prefix = format!("{name}: ");
        Ok(description
            .split_once(&prefix)
            .ok_or("pinned circuit metadata changed")?
            .1
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .ok_or("column count missing")?
            .parse()?)
    };
    let degree = meta.degree();
    // EvaluationDomain::new in pinned Halo2 extends to cover (degree - 1) * n.
    let extension = (degree - 1).next_power_of_two();
    Ok(
        serde_json::json!({"scope":"static native row circuit profile, not measured memory or a proof","layout":layout,
        "k":K,"constraint_degree":degree,"extended_domain_factor":extension,
        "advice_columns":count("num_advice_columns")?,"fixed_columns_before_selector_compression":count("num_fixed_columns")?,
        "instance_columns":count("num_instance_columns")?,"blinding_factors":meta.blinding_factors(),
        "configured_system_sha256":format!("{:x}",Sha256::digest(description.as_bytes())),
        "proof_generated":false,"witness_or_private_files_read":false,"working_circuit_changed":false}),
    )
}

/// Read native note opening from the exact output; no independently supplied
/// amount or recipient enters the native commitment computation.
pub fn binding_from_note(row: &Row, note: &Note) -> Result<Binding> {
    if note.version() != NoteVersion::V3 {
        return Err("binding expects an Ironwood V3 note".into());
    }
    let gd = note.recipient().g_d();
    let pkd = note.recipient().pk_d().inner();
    if row.gd != gd.to_bytes() || row.pkd != pkd.to_bytes() || row.amount != note.value().inner() {
        return Err("row recipient/amount differs from native note".into());
    }
    let rho = note.rho();
    let psi = note.rseed().psi(&rho);
    let rcm = note
        .rseed()
        .rcm_v3(&rho, &gd, &pkd, row.amount, &psi)
        .inner();
    let bytes = row.encode();
    Ok(Binding {
        gd: Value::known(gd.to_affine()),
        pkd: Value::known(pkd.to_affine()),
        value: Value::known(row.amount),
        rho: Value::known(rho.into_inner()),
        psi: Value::known(psi),
        rcm: Value::known(rcm),
        prefix: std::array::from_fn(|j| {
            Value::known(u32::from_be_bytes(
                bytes[16 + j * 4..20 + j * 4].try_into().unwrap(),
            ))
        }),
        salt: std::array::from_fn(|j| {
            Value::known(u32::from_be_bytes(
                bytes[192 + j * 4..196 + j * 4].try_into().unwrap(),
            ))
        }),
    })
}

pub fn row_public(statement: &NativeBatchStatement, row: usize) -> Result<Vec<pallas::Base>> {
    if row >= 6 {
        return Err("row outside six-row statement".into());
    }
    let cmx = ExtractedNoteCommitment::from_bytes(&statement.cmxs[row])
        .into_option()
        .ok_or("noncanonical native commitment")?;
    let mut public: Vec<_> = statement.row_hashes[row]
        .map(|x| pallas::Base::from(x as u64))
        .into();
    public.extend([
        cmx.inner(),
        pallas::Base::from(statement.action_indices[row] as u64),
    ]);
    Ok(public)
}

pub struct RowProver {
    params: Params<vesta::Affine>,
    pk: ProvingKey<vesta::Affine>,
}

/// Experimental shared-column key, selected by the caller's explicit mode.
/// Existing funded statements continue using RowProver and its original VK.
pub struct SharedRowProver {
    params: Params<vesta::Affine>,
    pk: ProvingKey<vesta::Affine>,
}

pub enum LayoutProver {
    Working(RowProver),
    Shared(SharedRowProver),
}
pub struct ProfiledRowProof {
    pub proof: Vec<u8>,
    pub create_seconds: f64,
    pub verify_seconds: f64,
}
impl LayoutProver {
    /// Instrument the same fixed shared18 key construction, without importing or
    /// serializing a key. Circuit, parameters and public statements are unchanged.
    pub fn shared_profiled() -> Result<(Self, serde_json::Value)> {
        let (prover, timing) = SharedRowProver::new_profiled()?;
        Ok((Self::Shared(prover), timing))
    }
    pub fn shared_prove_profiled(
        &self,
        binding: Binding,
        public: &[pallas::Base],
    ) -> Result<ProfiledRowProof> {
        match self {
            Self::Shared(prover) => prover.prove_profiled(binding, public),
            Self::Working(_) => Err("profiled worker requires fixed shared18 layout".into()),
        }
    }
    pub fn new(layout: RowLayout) -> Result<Self> {
        Ok(match layout {
            RowLayout::Working33 => Self::Working(RowProver::new()?),
            RowLayout::Shared18 => Self::Shared(SharedRowProver::new()?),
        })
    }
    pub fn prove(&self, binding: Binding, public: &[pallas::Base]) -> Result<Vec<u8>> {
        match self {
            Self::Working(prover) => prover.prove(binding, public),
            Self::Shared(prover) => prover.prove(binding, public),
        }
    }
    pub fn verify(&self, proof: &[u8], public: &[pallas::Base]) -> Result<()> {
        match self {
            Self::Working(prover) => prover.verify(proof, public),
            Self::Shared(prover) => prover.verify(proof, public),
        }
    }
}
impl SharedRowProver {
    pub fn new() -> Result<Self> {
        Ok(Self::new_profiled()?.0)
    }
    fn new_profiled() -> Result<(Self, serde_json::Value)> {
        let started = std::time::Instant::now();
        let params = Params::new(K);
        let params_seconds = started.elapsed().as_secs_f64();
        let empty = SharedPoseidonBinding::default();
        let vk_start = std::time::Instant::now();
        let vk = keygen_vk(&params, &empty)?;
        let vk_seconds = vk_start.elapsed().as_secs_f64();
        let pk_start = std::time::Instant::now();
        let pk = keygen_pk(&params, vk, &empty)?;
        let pk_seconds = pk_start.elapsed().as_secs_f64();
        Ok((
            Self { params, pk },
            serde_json::json!({
                "params_seconds":params_seconds,"vk_seconds":vk_seconds,"pk_seconds":pk_seconds,
                "total_seconds":started.elapsed().as_secs_f64()
            }),
        ))
    }
    pub fn prove(&self, binding: Binding, public: &[pallas::Base]) -> Result<Vec<u8>> {
        Ok(self.prove_profiled(binding, public)?.proof)
    }
    fn prove_profiled(
        &self,
        binding: Binding,
        public: &[pallas::Base],
    ) -> Result<ProfiledRowProof> {
        let started = std::time::Instant::now();
        let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(Vec::new());
        create_proof(
            &self.params,
            &self.pk,
            &[SharedPoseidonBinding(binding)],
            &[&[public]],
            OsRng,
            &mut transcript,
        )?;
        let proof = transcript.finalize();
        let create_seconds = started.elapsed().as_secs_f64();
        let verify_start = std::time::Instant::now();
        self.verify(&proof, public)?;
        Ok(ProfiledRowProof {
            proof,
            create_seconds,
            verify_seconds: verify_start.elapsed().as_secs_f64(),
        })
    }
    pub fn verify(&self, proof: &[u8], public: &[pallas::Base]) -> Result<()> {
        if proof.len() > 65536 || public.len() != 10 {
            return Err("bounded shared row statement required".into());
        }
        let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(proof);
        verify_proof(
            &self.params,
            self.pk.get_vk(),
            SingleVerifier::new(&self.params),
            &[&[public]],
            &mut transcript,
        )?;
        Ok(())
    }
}

/// Explicit fixed shared-layout VK. It cannot consume a caller-supplied key.
/// Compatibility replay also checks altered inputs/proof using this same VK.
pub fn verify_shared_one(proof: &[u8], public: &[pallas::Base]) -> Result<()> {
    if proof.len() > 65536 || public.len() != 10 {
        return Err("bounded shared row statement required".into());
    }
    let params = Params::new(K);
    let vk = keygen_vk(&params, &SharedPoseidonBinding::default())?;
    let verify = |proof: &[u8], public: &[pallas::Base]| {
        let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(proof);
        verify_proof(
            &params,
            &vk,
            SingleVerifier::new(&params),
            &[&[public]],
            &mut transcript,
        )
    };
    verify(proof, public)?;
    for i in [0, 8, 9] {
        let mut changed = public.to_vec();
        changed[i] += pallas::Base::from(1);
        if verify(proof, &changed).is_ok() {
            return Err("shared replay altered input accepted".into());
        }
    }
    let mut changed = proof.to_vec();
    changed[0] ^= 1;
    if verify(&changed, public).is_ok() {
        return Err("shared replay altered proof accepted".into());
    }
    Ok(())
}
impl RowProver {
    /// One key for all six rows. The circuit is chosen here, never from a proof.
    pub fn new() -> Result<Self> {
        let params = Params::new(K);
        let empty = PoseidonBinding::default().without_witnesses();
        let vk = keygen_vk(&params, &empty)?;
        let pk = keygen_pk(&params, vk, &empty)?;
        Ok(Self { params, pk })
    }
    pub fn prove(&self, row: Binding, public: &[pallas::Base]) -> Result<Vec<u8>> {
        let circuit = PoseidonBinding(row);
        let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(Vec::new());
        create_proof(
            &self.params,
            &self.pk,
            &[circuit],
            &[&[public]],
            OsRng,
            &mut transcript,
        )?;
        let proof = transcript.finalize();
        self.verify(&proof, public)?;
        Ok(proof)
    }
    pub fn verify(&self, proof: &[u8], public: &[pallas::Base]) -> Result<()> {
        let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(proof);
        verify_proof(
            &self.params,
            self.pk.get_vk(),
            SingleVerifier::new(&self.params),
            &[&[public]],
            &mut transcript,
        )?;
        Ok(())
    }
}

/// Public-only independent verification against the fixed trusted circuit key.
/// Callers must additionally compare native_actions with the actual transaction.
pub fn verify_set(statement: &NativeBatchStatement, proofs: &[Vec<u8>]) -> Result<()> {
    verify_set_for_layout(RowLayout::Working33, statement, proofs)
}

pub fn verify_set_for_layout(
    layout: RowLayout,
    statement: &NativeBatchStatement,
    proofs: &[Vec<u8>],
) -> Result<()> {
    match layout {
        RowLayout::Working33 => verify_set_with::<PoseidonBinding>(statement, proofs),
        RowLayout::Shared18 => verify_set_with::<SharedPoseidonBinding>(statement, proofs),
    }
}

fn verify_set_with<C: Circuit<pallas::Base> + Default>(
    statement: &NativeBatchStatement,
    proofs: &[Vec<u8>],
) -> Result<()> {
    if proofs.len() != 6 || proofs.iter().any(|p| p.len() > 65536) {
        return Err("expected six bounded native row proofs".into());
    }
    link_four_bid_statements(
        &statement.row_hashes,
        &statement.bindings(),
        &statement.native_actions,
    )
    .map_err(|_| "invalid native action mapping")?;
    let params = Params::new(K);
    let vk = keygen_vk(&params, &C::default())?;
    for (i, proof) in proofs.iter().enumerate() {
        let public = row_public(statement, i)?;
        let mut transcript =
            Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(proof.as_slice());
        verify_proof(
            &params,
            &vk,
            SingleVerifier::new(&params),
            &[&[&public]],
            &mut transcript,
        )?;
    }
    Ok(())
}

/// Single-row compatibility replay under the fixed circuit, with no private note.
/// The caller must compare cmx/index with an independently verified native tx.
pub fn verify_one(proof: &[u8], public: &[pallas::Base]) -> Result<()> {
    if proof.len() > 65536 || public.len() != 10 {
        return Err("expected one bounded row proof and ten public inputs".into());
    }
    let params = Params::new(K);
    let vk = keygen_vk(&params, &PoseidonBinding::default())?;
    let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(proof);
    verify_proof(
        &params,
        &vk,
        SingleVerifier::new(&params),
        &[&[public]],
        &mut transcript,
    )?;
    Ok(())
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn layout_metadata_accepts_only_compiled_modes_and_preserves_legacy_default() {
        let dir = std::env::temp_dir().join(format!("darkcall-layout-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        assert_eq!(read_layout(&dir).unwrap(), RowLayout::Working33);
        write_layout(&dir, RowLayout::Shared18).unwrap();
        assert_eq!(read_layout(&dir).unwrap(), RowLayout::Shared18);
        assert!(write_layout(&dir, RowLayout::Working33).is_err());
        let path = dir.join("native-layout.json");
        for value in [
            serde_json::json!({"format":1,"layout":"upload-your-own-vk"}),
            serde_json::json!({"format":2,"layout":SHARED_LAYOUT}),
            serde_json::json!({"format":1,"layout":SHARED_LAYOUT,"private_witness":[]}),
            serde_json::json!({"format":1}),
        ] {
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(read_layout(&dir).is_err());
        }
        std::fs::remove_file(&path).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("missing-layout", &path).unwrap();
            assert!(read_layout(&dir).is_err());
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn existing_circuit_configuration_is_unchanged_and_shared_layout_is_distinct() {
        let old = circuit_profile().unwrap();
        assert_eq!(
            old["configured_system_sha256"],
            "22fe5ec710bd191052c1b208e84f70c84f3612a2b8d4aae3e57654b1fa1bd4cd"
        );
        assert_eq!(old["advice_columns"], 33);
        let shared = shared_circuit_profile().unwrap();
        assert_eq!(shared["advice_columns"], 18);
        assert_eq!(shared["constraint_degree"], old["constraint_degree"]);
        assert_ne!(
            shared["configured_system_sha256"],
            old["configured_system_sha256"]
        );
    }
}
