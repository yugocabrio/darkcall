//! Native encrypted memo transport component. Fabricated notes only; this
//! transaction does not settle or publicly bind an auction's six output rows.
use crate::{
    Result, auction,
    interop_export::{SDK, write},
    native,
};
use orchard::{
    keys::{FullViewingKey, Scope},
    note::{ExtractedNoteCommitment, NoteVersion},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use zcash_protocol::{consensus::BranchId, memo::MemoBytes};

const RULE: &str = "synthetic-native-receipt-transport-v1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: u32,
    amounts: [u64; 4],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipient {
    format: u32,
    rule: String,
    sdk: String,
    branch: String,
    txid: String,
    action_index: usize,
    amount: u64,
    viewing_key: Vec<u8>,
}
pub(crate) struct Transport {
    directory: PathBuf,
    pub amounts: [u64; 4],
    pub memos: [MemoBytes; 4],
}
impl Transport {
    pub fn load(path: &Path) -> Result<Self> {
        let directory = auction::directory(path)?;
        let request: Request = serde_json::from_slice(&auction::read(
            &directory.join("receipt-transport-request.json"),
            4096,
        )?)
        .map_err(|_| "receipt transport request schema")?;
        if request.format != 1 || request.amounts.iter().any(|a| *a > 100) {
            return Err("expected bounded synthetic receipt amounts".into());
        }
        // Opaque versioned memo bytes, not an interpretation or verification
        // of the opening. Field canonicality and fixed proof/context validation
        // belong to the recipient's opening client.
        let memos = (0..4)
            .map(|i| -> Result<MemoBytes> {
                let bytes = auction::read(&directory.join(format!("ticket-{i}.memo")), 512)?;
                if bytes.len() != 512
                    || &bytes[..9] != b"\xffDCTKT001"
                    || bytes[201..].iter().any(|v| *v != 0)
                {
                    return Err("receipt transport requires versioned private memo".into());
                }
                MemoBytes::from_bytes(&bytes).map_err(|_| "receipt memo size".into())
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| "four receipt memos")?;
        Ok(Self {
            directory,
            amounts: request.amounts,
            memos,
        })
    }
    pub fn persist(
        &self,
        transaction: &[u8],
        txid: &str,
        receivers: &[FullViewingKey],
        indices: &[usize],
    ) -> Result<()> {
        if receivers.len() != 4 || indices.len() != 4 {
            return Err("receipt transport shape".into());
        }
        for i in 0..4 {
            let recipient = Recipient {
                format: 1,
                rule: RULE.into(),
                sdk: SDK.into(),
                branch: "77190ad9".into(),
                txid: txid.into(),
                action_index: indices[i],
                amount: self.amounts[i],
                viewing_key: receivers[i].to_bytes().to_vec(),
            };
            write(
                &self.directory.join(format!("private-recipient-{i}.json")),
                &serde_json::to_vec(&recipient).map_err(|_| "private recipient encoding")?,
            )?;
        }
        write(
            &self.directory.join("synthetic-receipt-transport.bin"),
            transaction,
        )
    }
}

/// Each recipient directory contains only their own viewing key/expected
/// identity and the public transaction. It need not contain operator pads,
/// other viewing keys, spending keys or the producer's PCZT.
pub fn open(path: &Path, index: usize, out: &Path) -> Result<serde_json::Value> {
    let directory = auction::directory(path)?;
    if index >= 4
        || out
            .parent()
            .ok_or("memo output parent")?
            .canonicalize()
            .map_err(|_| "memo output parent missing")?
            != directory
    {
        return Err("expected own ignored recipient directory and index".into());
    }
    let recipient: Recipient = serde_json::from_slice(&auction::read(
        &directory.join(format!("private-recipient-{index}.json")),
        4096,
    )?)
    .map_err(|_| "recipient identity schema")?;
    if recipient.format != 1
        || recipient.rule != RULE
        || recipient.sdk != SDK
        || recipient.branch != "77190ad9"
        || recipient.action_index >= 4
        || recipient.amount > 100
    {
        return Err("recipient native identity mismatch".into());
    }
    let fvk = FullViewingKey::from_bytes(
        recipient
            .viewing_key
            .as_slice()
            .try_into()
            .map_err(|_| "recipient viewing key length")?,
    )
    .ok_or("recipient viewing key encoding")?;
    let bytes = auction::read(&directory.join("synthetic-receipt-transport.bin"), 100000)?;
    // Authenticate the complete native transaction before consuming ciphertext.
    // This verifies native consensus authorization, not application policy or
    // chain inclusion. The expected identity comes from the recipient fixture.
    let tx = native::verify_bytes(&bytes, BranchId::Nu7, &mut rand::rng())?;
    let bundle = tx.ironwood_bundle().ok_or("receipt native bundle")?;
    if tx.txid().to_string() != recipient.txid || bundle.actions().len() != 4 {
        return Err("recipient transaction identity/shape differs".into());
    }
    let (note, address, memo) = bundle
        .decrypt_output_with_key(recipient.action_index, &fvk.to_ivk(Scope::External))
        .ok_or("recipient cannot decrypt private receipt")?;
    if note.value().inner() != recipient.amount
        || note.version() != NoteVersion::V3
        || address != fvk.address_at(0u32, Scope::External)
        || ExtractedNoteCommitment::from(note.commitment())
            != *bundle.actions()[recipient.action_index].cmx()
    {
        return Err("recipient receipt note mismatch".into());
    }
    write(out, &memo)?;
    Ok(
        serde_json::json!({"scope":"recipient native memo decryption only; synthetic transport component",
        "native_proof_and_signatures_verified":true,"own_viewing_key_only":true,"other_ticket_pads_read":false,
        "amount":note.value().inner(),"memo_bytes":memo.len(),"txid":tx.txid().to_string(),
        "auction_application_binding_verified":false,"ticket_opening_verified_here":false,
        "chain_inclusion_verified":false}),
    )
}
