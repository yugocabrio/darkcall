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
const FUNDED_RULE: &str = "actual-native-six-output-auction-receipt-v1";
const AUCTION_RULE: &str = "synthetic-native-six-output-auction-receipt-v1";
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
    open_inner(path, index, out, false, false)
}

pub fn open_auction(path: &Path, index: usize, out: &Path) -> Result<serde_json::Value> {
    open_inner(path, index, out, true, false)
}

pub fn open_funded(path: &Path, index: usize, out: &Path) -> Result<serde_json::Value> {
    open_inner(path, index, out, true, true)
}
/// Explicit local recipient setup: derive only this wallet's viewing identity.
/// This export may read that actor's ordinary spending key; subsequent receipt
/// opening consumes only the exported FVK, never that key or another actor's FVK.
pub fn export_funded(path: &Path, index: usize, wallet: &Path) -> Result<serde_json::Value> {
    let directory = auction::directory(path)?;
    if index >= 4 {
        return Err("funded own ticket index".into());
    }
    let identity: crate::auction::Identity = serde_json::from_slice(&auction::read(
        &directory.join("nu7-native-identity.json"),
        16384,
    )?)
    .map_err(|_| "funded recipient native identity")?;
    if identity.scope != crate::funded_prepare::SCOPE
        || identity.sdk != SDK
        || identity.branch != "77190ad9"
    {
        return Err("funded recipient scope/pin mismatch".into());
    }
    let statement: serde_json::Value = serde_json::from_slice(&auction::read(
        &directory.join("native-public.json"),
        32768,
    )?)
    .map_err(|_| "funded recipient action map")?;
    let fvk = crate::regtest_wallet::viewing_key(wallet)?;
    let bytes = auction::read(&directory.join("funded-nu7-transaction.bin"), 100000)?;
    let tx = native::verify_bytes(&bytes, BranchId::Nu7, &mut rand::rng())?;
    let action_index = usize::try_from(
        statement["action_indices"][index]
            .as_u64()
            .ok_or("funded recipient action")?,
    )
    .map_err(|_| "funded recipient action range")?;
    if tx.txid().to_string() != identity.txid || action_index >= 6 {
        return Err("funded recipient transaction identity".into());
    }
    let (note, address, _) = tx
        .ironwood_bundle()
        .ok_or("funded native bundle")?
        .decrypt_output_with_key(action_index, &fvk.to_ivk(Scope::External))
        .ok_or("funded actor cannot decrypt own receipt")?;
    if address != fvk.address_at(0u32, Scope::External) || note.value().inner() > 100 {
        return Err("funded own recipient address/amount".into());
    }
    write(
        &directory.join(format!("private-funded-recipient-{index}.json")),
        &serde_json::to_vec(&Recipient {
            format: 1,
            rule: FUNDED_RULE.into(),
            sdk: SDK.into(),
            branch: "77190ad9".into(),
            txid: identity.txid,
            action_index,
            amount: note.value().inner(),
            viewing_key: fvk.to_bytes().to_vec(),
        })
        .map_err(|_| "funded own recipient encoding")?,
    )?;
    Ok(
        serde_json::json!({"own_viewing_identity_exported":true,"actor_setup_may_read_own_spending_key":true,
        "other_wallets_read":false,"private_key_logged":false}),
    )
}

/// Producer-side export. The recipient reader subsequently reads only its one
/// record. This does not confer registration or chain-inclusion authentication.
pub fn persist_auction(
    directory: &Path,
    txid: &str,
    receivers: &[FullViewingKey],
    indices: &[u32; 6],
    amounts: &[u64; 6],
) -> Result<()> {
    if receivers.len() != 6 {
        return Err("six auction receivers required".into());
    }
    for i in 0..4 {
        let recipient = Recipient {
            format: 1,
            rule: AUCTION_RULE.into(),
            sdk: SDK.into(),
            branch: "77190ad9".into(),
            txid: txid.into(),
            action_index: indices[i] as usize,
            amount: amounts[i],
            viewing_key: receivers[i].to_bytes().to_vec(),
        };
        write(
            &directory.join(format!("private-auction-recipient-{i}.json")),
            &serde_json::to_vec(&recipient).map_err(|_| "auction recipient encoding")?,
        )?;
    }
    Ok(())
}

fn open_inner(
    path: &Path,
    index: usize,
    out: &Path,
    auction_receipt: bool,
    funded: bool,
) -> Result<serde_json::Value> {
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
    let prefix = if funded {
        "private-funded-recipient"
    } else if auction_receipt {
        "private-auction-recipient"
    } else {
        "private-recipient"
    };
    let actions = if auction_receipt { 6 } else { 4 };
    let recipient: Recipient = serde_json::from_slice(&auction::read(
        &directory.join(format!("{prefix}-{index}.json")),
        4096,
    )?)
    .map_err(|_| "recipient identity schema")?;
    if recipient.format != 1
        || recipient.rule
            != if funded {
                FUNDED_RULE
            } else if auction_receipt {
                AUCTION_RULE
            } else {
                RULE
            }
        || recipient.sdk != SDK
        || recipient.branch != "77190ad9"
        || recipient.action_index >= actions
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
    let filename = if funded {
        "funded-nu7-transaction.bin"
    } else if auction_receipt {
        "synthetic-nu7-transaction.bin"
    } else {
        "synthetic-receipt-transport.bin"
    };
    let bytes = auction::read(&directory.join(filename), 100000)?;
    // Authenticate the complete native transaction before consuming ciphertext.
    // This verifies native consensus authorization, not application policy or
    // chain inclusion. The expected identity comes from the recipient fixture.
    let tx = native::verify_bytes(&bytes, BranchId::Nu7, &mut rand::rng())?;
    let bundle = tx.ironwood_bundle().ok_or("receipt native bundle")?;
    if tx.txid().to_string() != recipient.txid || bundle.actions().len() != actions {
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
        serde_json::json!({"scope":if funded {"recipient native memo decryption; chain/proof policy checked separately"} else {"recipient native memo decryption only; synthetic funding"},
        "native_proof_and_signatures_verified":true,"own_viewing_key_only":true,"other_ticket_pads_read":false,
        "amount":note.value().inner(),"memo_bytes":memo.len(),"txid":tx.txid().to_string(),
        "receipt_in_six_output_transaction":auction_receipt,
        "auction_application_binding_verified":false,"ticket_opening_verified_here":false,
        "chain_inclusion_verified":false}),
    )
}
