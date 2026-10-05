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
impl SharedRowProver {
    pub fn new() -> Result<Self> {
        let params = Params::new(K);
        let empty = SharedPoseidonBinding::default();
        let vk = keygen_vk(&params, &empty)?;
        let pk = keygen_pk(&params, vk, &empty)?;
        Ok(Self { params, pk })
    }
    pub fn prove(&self, binding: Binding, public: &[pallas::Base]) -> Result<Vec<u8>> {
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
        self.verify(&proof, public)?;
        Ok(proof)
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
    let vk = keygen_vk(&params, &PoseidonBinding::default())?;
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
