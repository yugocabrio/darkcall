//! Native V3 witness semantics across the new SDK and working binding stack.
//! Separate Rust types, canonical bytes only. No funding/broadcast or proof.
use crate::Result;
use ff::{Field, PrimeField};
use orchard::{
    Note,
    keys::{FullViewingKey, Scope, SpendingKey},
    note::{NoteVersion, RandomSeed, Rho},
};
use pasta_curves::pallas;
use rand::Rng;

fn old_note(note: &Note, amount: u64) -> Result<orchard_old::Note> {
    let recipient =
        orchard_old::Address::from_raw_address_bytes(&note.recipient().to_raw_address_bytes())
            .into_option()
            .ok_or("new native recipient cannot be decoded by working binding stack")?;
    let rho = orchard_old::note::Rho::from_bytes(&note.rho().to_bytes())
        .into_option()
        .ok_or("rho encoding mismatch")?;
    let seed = orchard_old::note::RandomSeed::from_bytes(*note.rseed().as_bytes(), &rho)
        .into_option()
        .ok_or("rseed encoding mismatch")?;
    orchard_old::Note::from_parts(
        recipient,
        orchard_old::value::NoteValue::from_raw(amount),
        rho,
        seed,
        orchard_old::note::NoteVersion::V3,
    )
    .into_option()
    .ok_or("new native opening rejected by working V3 note semantics".into())
}

pub fn gate() -> Result<serde_json::Value> {
    let started = std::time::Instant::now();
    let mut rng = rand::rng();
    let key = loop {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        if let Some(key) = SpendingKey::from_bytes(bytes).into_option() {
            break key;
        }
    };
    let fvk = FullViewingKey::from(&key);
    let old_fvk = orchard_old::keys::FullViewingKey::from_bytes(&fvk.to_bytes())
        .ok_or("FVK encoding mismatch")?;
    let amounts = [0, 1, 100, 15100, 30000, 1_000_000, 2_100_000_000_000_000];
    for (i, amount) in amounts.into_iter().enumerate() {
        let scope = if i % 2 == 0 {
            Scope::External
        } else {
            Scope::Internal
        };
        let old_scope = if i % 2 == 0 {
            orchard_old::keys::Scope::External
        } else {
            orchard_old::keys::Scope::Internal
        };
        let address = fvk.address_at(i as u32, scope);
        if address.to_raw_address_bytes()
            != old_fvk
                .address_at(i as u32, old_scope)
                .to_raw_address_bytes()
        {
            return Err("FVK address derivation changed".into());
        }
        let note = loop {
            let rho = Rho::from_bytes(&pallas::Base::random(&mut rng).to_repr()).unwrap();
            let mut bytes = [0; 32];
            rng.fill_bytes(&mut bytes);
            if let Some(seed) = RandomSeed::from_bytes(bytes, &rho).into_option()
                && let Some(note) = Note::from_parts(
                    address,
                    orchard::value::NoteValue::from_raw(amount),
                    rho,
                    seed,
                    NoteVersion::V3,
                )
                .into_option()
            {
                break note;
            }
        };
        let old = old_note(&note, amount)?;
        let actual = orchard::note::ExtractedNoteCommitment::from(note.commitment()).to_bytes();
        if orchard_old::note::ExtractedNoteCommitment::from(old.commitment()).to_bytes() != actual
            || old.nullifier(&old_fvk).to_bytes() != note.nullifier(&fvk).to_bytes()
        {
            return Err("native V3 commitment/nullifier semantics changed".into());
        }
        let psi = note.rseed().psi(&note.rho());
        let old_psi = old.rseed().psi(&old.rho());
        let rcm = note
            .rseed()
            .rcm_v3(
                &note.rho(),
                &address.g_d(),
                &address.pk_d().inner(),
                amount,
                &psi,
            )
            .inner();
        let old_address = old.recipient();
        let old_rcm = old
            .rseed()
            .rcm_v3(
                &old.rho(),
                &old_address.g_d(),
                &old_address.pk_d().inner(),
                amount,
                &old_psi,
            )
            .inner();
        if ff_old::PrimeField::to_repr(&old_psi).as_ref() != psi.to_repr().as_ref()
            || ff_old::PrimeField::to_repr(&old_rcm).as_ref() != rcm.to_repr().as_ref()
        {
            return Err("V3 native binding witness bytes changed".into());
        }
        let changed = old_note(&note, amount + 1)?;
        if orchard_old::note::ExtractedNoteCommitment::from(changed.commitment()).to_bytes()
            == actual
        {
            return Err("changed amount accepted as original native commitment".into());
        }
    }
    Ok(
        serde_json::json!({"scope":"synthetic native V3 semantic byte interoperability; no auction proof or chain funding",
        "old_orchard_pin":"42015f15733b95e179189628d6fa34d92b5b3b8b","new_orchard":"0.16.0",
        "cases":amounts.len(),"external_and_internal_address_derivation_match":true,"canonical_fvk_and_note_encodings_match":true,
        "native_commitments_and_nullifiers_match":true,"psi_and_rcm_v3_witness_bytes_match":true,"changed_amount_rejected":true,
        "rust_field_or_group_types_shared":false,"private_material_emitted":false,"main_workspace_pins_changed":false,
        "native_or_application_proof_generated":false,"broadcast":false,"seconds":started.elapsed().as_secs_f64()}),
    )
}
