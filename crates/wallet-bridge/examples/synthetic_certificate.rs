//! Offline gate ONLY: fabricate funding, decrypt real Ironwood ciphertexts,
//! authenticate client intent, issue receipts and certify an existing public
//! MPC statement. Admission observations are assertions, not chain evidence.
use darkcall_input_tool::decimal_field_bytes;
use darkcall_wallet_bridge::{
    claim::{Claim, DepositMemo, ExpectedFunding, Instructions, verify_output},
    final_book::{Certificate, Endorsement, FinalBook, HASH_RULE, Manifest},
    journal::{Context, Event, Journal},
    receipt::{AcceptedReceipt, Receipt},
};
use ed25519_dalek::SigningKey;
use incrementalmerkletree::Hashable;
use orchard::{
    Note,
    builder::BundleType,
    keys::{FullViewingKey, Scope, SpendingKey},
    note::{NoteVersion, RandomSeed, Rho},
    tree::{MerkleHashOrchard, MerklePath},
    value::NoteValue,
};
use pczt::roles::{creator::Creator, io_finalizer::IoFinalizer};
use rand::{RngCore, rngs::OsRng};
use serde::Deserialize;
use std::{fs, path::Path};
use zcash_primitives::transaction::{
    builder::{BuildConfig, Builder},
    fees::zip317::FeeRule,
};
use zcash_protocol::{
    consensus::{NetworkUpgrade, Parameters, TEST_NETWORK},
    memo::MemoBytes,
    value::Zatoshis,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    condition_digest: [u8; 32],
    ids: [[u8; 32]; 4],
    active: [bool; 4],
    units: u32,
    reserve: u64,
    budget: u64,
    processing_fee: u64,
    commitments: [String; 4],
    pair_commitments: [[String; 3]; 4],
    root: String,
}
fn fresh() -> [u8; 32] {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "usage: synthetic_certificate PUBLIC_BOOK MPC_PUBLIC_INPUT NEW_LOCAL_DIRECTORY".into(),
        );
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let local = root.join(".local").canonicalize()?;
    let book_path = Path::new(&args[0]).canonicalize()?;
    let mpc_path = Path::new(&args[1]).canonicalize()?;
    let out = Path::new(&args[2]);
    if !book_path.starts_with(&local)
        || !mpc_path.starts_with(&local)
        || !out
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(&local)
        || out.exists()
    {
        return Err("only ignored offline sources and a new .local directory are allowed".into());
    }
    if fs::metadata(&book_path)?.len() > 16384 || fs::metadata(&mpc_path)?.len() != 232 * 32 {
        return Err("unexpected public artifact shape".into());
    }
    let book: Book = serde_json::from_slice(&fs::read(book_path)?)?;
    if book.active != [true; 4]
        || book.ids != {
            let mut ids = book.ids;
            ids.sort();
            ids
        }
    {
        return Err("expected canonical four-active public book".into());
    }
    let encoded = fs::read(mpc_path)?;
    let receivers: [[u8; 32]; 6] =
        std::array::from_fn(|i| encoded[(169 + i) * 32..(170 + i) * 32].try_into().unwrap());
    let keys = std::array::from_fn(|_| SigningKey::from_bytes(&fresh()));
    let manifest = Manifest {
        context: Context {
            format: 1,
            network_genesis: hex::decode(
                "05a60a92d99d85997cce3b87616c089f6124d7342af37106edc76126334a2c38",
            )?
            .try_into()
            .unwrap(),
            conditions_hash: book.condition_digest,
            hash_rule: HASH_RULE.into(),
            units: book.units,
            reserve: book.reserve,
            budget: book.budget,
            processing_fee: book.processing_fee,
        },
        operator_keys: keys
            .each_ref()
            .map(|key: &SigningKey| key.verifying_key().to_bytes()),
        closes_at_utc: 1_792_924_800, // Synthetic declared cutoff, not event timing evidence.
        seller_receiver_commitment: receivers[4],
        operator_receiver_commitment: receivers[5],
    };
    let operators = manifest.operators()?;
    let required = book
        .budget
        .checked_add(book.processing_fee)
        .ok_or("deposit overflow")?;
    let input_value = required
        .checked_mul(4)
        .and_then(|v| v.checked_add(20_000))
        .ok_or("input overflow")?;
    let escrow = loop {
        if let Some(sk) = SpendingKey::from_bytes(fresh()).into_option() {
            break FullViewingKey::from(&sk);
        }
    };
    let rho = Rho::from_bytes(&[1; 32]).unwrap();
    let seed = loop {
        if let Some(seed) = RandomSeed::from_bytes(fresh(), &rho).into_option() {
            break seed;
        }
    };
    let input = Note::from_parts(
        escrow.address_at(0u32, Scope::External),
        NoteValue::from_raw(input_value),
        rho,
        seed,
        NoteVersion::V3,
    )
    .unwrap();
    let path = MerklePath::from_parts(
        0,
        std::array::from_fn(|level| MerkleHashOrchard::empty_root((level as u8).into())),
    );
    let height = TEST_NETWORK
        .activation_height(NetworkUpgrade::Nu6_3)
        .unwrap()
        + 1;
    let mut builder = Builder::new(
        TEST_NETWORK,
        height,
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
            ironwood_anchor: Some(path.root(input.commitment().into())),
            orchard_pool_bundle_type: BundleType::DEFAULT,
        },
    );
    builder.add_ironwood_spend::<std::convert::Infallible>(escrow.clone(), input, path)?;
    let clients: [SigningKey; 4] = std::array::from_fn(|_| SigningKey::from_bytes(&fresh()));
    let memos: [DepositMemo; 4] = std::array::from_fn(|i| DepositMemo {
        network_genesis: manifest.context.network_genesis,
        conditions: book.condition_digest,
        ticket: book.ids[i],
        client_key: clients[i].verifying_key().to_bytes(),
        nonce: fresh(),
    });
    for memo in &memos {
        builder.add_ironwood_output::<std::convert::Infallible>(
            None,
            escrow.address_at(0u32, Scope::External),
            Zatoshis::from_u64(required)?,
            MemoBytes::from_bytes(&memo.encode()?)?,
        )?;
    }
    let parts = builder.build_for_pczt(OsRng, &FeeRule::standard())?;
    let indices: [usize; 4] =
        std::array::from_fn(|i| parts.ironwood_meta.output_action_index(i).unwrap());
    let effects = IoFinalizer::new(
        Creator::build_from_parts(parts.pczt_parts).ok_or("synthetic PCZT creation failed")?,
    )
    .finalize_io()
    .map_err(|e| format!("synthetic IO finalization: {e:?}"))?
    .into_effects()
    .map_err(|e| format!("synthetic effects extraction: {e:?}"))?;
    fs::create_dir(out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(out, fs::Permissions::from_mode(0o700))?;
    }
    let mut journals: [Journal; 3] = std::array::from_fn(|i| {
        Journal::open(
            &out.join(format!("private-node-{i}.jsonl")),
            &manifest.context,
        )
        .unwrap()
    });
    let mut receipts = Vec::new();
    for i in 0..4 {
        let pairs = book.pair_commitments[i]
            .each_ref()
            .map(|x| decimal_field_bytes(x));
        let pairs = pairs
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let instructions = Instructions {
            bid_commitment: decimal_field_bytes(&book.commitments[i])?,
            pair_commitments: pairs.try_into().unwrap(),
            refund_receiver_commitment: receivers[i],
        };
        let cmx = effects.ironwood_bundle().unwrap().actions()[indices[i]]
            .cmx()
            .to_bytes();
        let claim = Claim::sign(&memos[i], cmx, instructions, &clients[i])?;
        let verified = verify_output(
            &effects,
            indices[i],
            &escrow,
            &ExpectedFunding {
                network_genesis: manifest.context.network_genesis,
                conditions: book.condition_digest,
                ticket: book.ids[i],
                required_value: required,
            },
            &claim,
        )?;
        let reg = verified.registration();
        let mut endorsements = Vec::new();
        for operator in 0..3 {
            let j = &mut journals[operator];
            j.append(Event::Observe {
                note: reg.note,
                value: required,
                block: [1; 32],
                height: u32::from(height),
            })?;
            j.append(Event::Confirm {
                note: reg.note,
                block: [1; 32],
            })?;
            j.append(Event::Accept {
                ticket: reg.ticket,
                note: reg.note,
                bid_commitment: reg.bid_commitment,
                receiver_commitment: reg.receiver_commitment,
            })?;
            endorsements.push(Receipt::issue(
                j,
                &verified,
                &operators,
                operator,
                &keys[operator],
            )?);
        }
        receipts.push(AcceptedReceipt::combine(
            endorsements.try_into().ok().unwrap(),
            &operators,
            &manifest.context,
        )?);
    }
    let receipts: [AcceptedReceipt; 4] = receipts.try_into().ok().unwrap();
    let final_book = FinalBook::from_receipts(&manifest, &receipts)?;
    if final_book.accepted_root != decimal_field_bytes(&book.root)? {
        return Err("host root differs from existing public book".into());
    }
    let mut endorsements = Vec::new();
    for i in 0..3 {
        journals[i].append(Event::Freeze)?;
        endorsements.push(Endorsement::issue(
            &mut journals[i],
            &final_book,
            &manifest,
            i,
            &keys[i],
            manifest.closes_at_utc,
        )?);
    }
    let certificate = Certificate::combine(endorsements.try_into().ok().unwrap(), &manifest)?;
    certificate.verify_statement(&encoded, &manifest)?;
    for receipt in &receipts {
        certificate.check_inclusion(receipt, &manifest)?;
    }
    fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        out.join("final-book-certificate.json"),
        serde_json::to_vec_pretty(&certificate)?,
    )?;
    let summary = serde_json::json!({ "scope":"synthetic funding/claim/receipt final-root gate against existing public MPC inputs",
        "maximum_bids_read":false,"private_source_openings_read":false,"keys_persisted":false,
        "funding_ciphertexts_decrypted":4,"client_claim_signatures_verified":4,
        "operator_receipt_signatures":12,"final_book_signatures":3,"all_receipts_included":true,
        "actual_mpc_root_and_receivers_match":true,"chain_funding_or_finality_verified":false,
        "client_zk_and_pair_admission_reexecuted":false,"native_funding_proof_created":false,
        "funding_observations":"fabricated; not authoritative chain evidence" });
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
