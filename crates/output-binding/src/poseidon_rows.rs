//! Fixed six-row Poseidon/native proof set. This verifies application statements,
//! not chain inclusion, custody authorization, rseed derivation or ciphertext.
use crate::sha::{Binding, PoseidonBinding};
use darkcall_settlement_types::{NativeBatchStatement, Row, link_four_bid_statements};
use group::{Curve, GroupEncoding};
use halo2_proofs::{
    circuit::Value,
    plonk::{
        Circuit, ProvingKey, SingleVerifier, create_proof, keygen_pk, keygen_vk, verify_proof,
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

pub const K: u32 = 17;
pub const HASH_RULE: &str = "bn254-poseidon2-packed-row-v1";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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
