//! Admit actual scanned local-regtest deposits after rechecking client proofs and
//! delivered pairs. RPC inclusion/current unspent snapshot is guarded by driver.
use darkcall_input_tool::decimal_field_bytes;
use darkcall_wallet_bridge::{
    claim::{Claim, ExpectedFunding, WireClaim, verify_output},
    final_book::{Certificate, Endorsement, FinalBook, HASH_RULE, Manifest},
    journal::{Context, Event, Journal},
    receipt::{AcceptedReceipt, WireReceipt},
};
use ed25519_dalek::SigningKey;
use orchard::keys::{FullViewingKey, SpendingKey};
use rand::{RngCore, rngs::OsRng};
use serde::Deserialize;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::BranchId;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const GENESIS: &str = "029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327";
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
fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".local")
        .canonicalize()?;
    if !path.canonicalize()?.starts_with(root) {
        return Err("ignored paths required".into());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut f = options.open(path)?;
    let m = f.metadata()?;
    if !m.is_file() || m.len() > limit {
        return Err("input size/type".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if m.permissions().mode() & 0o077 != 0 || m.nlink() != 1 {
            return Err("private input permissions/links".into());
        }
    }
    let mut b = Vec::new();
    f.read_to_end(&mut b)?;
    Ok(b)
}
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut f = options.open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::File::open(path.parent().ok_or("output parent")?)?.sync_all()?;
    Ok(())
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn checked(command: &mut Command, log: &Path) -> Result<()> {
    let mut o = fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let f = o.open(log)?;
    let status = command
        .stdout(Stdio::from(f.try_clone()?))
        .stderr(Stdio::from(f))
        .status()?;
    if !status.success() {
        return Err("admission proof/share check failed; inspect private log".into());
    }
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    if args.len() == 4 && args[0] == "record" {
        let out = Path::new(&args[1]);
        let manifest: Manifest = serde_json::from_slice(&read(&out.join("manifest.json"), 16384)?)?;
        let mut encoded = hex::decode(&args[2])?;
        encoded.reverse();
        let effects: [u8; 32] = encoded.try_into().map_err(|_| "effects length")?;
        let event = match args[3].as_str() {
            "broadcast" => Event::Broadcast { effects },
            "finalize" => Event::Finalize { effects },
            _ => return Err("expected broadcast or finalize".into()),
        };
        for i in 0..3 {
            let mut journal = Journal::open(
                &out.join(format!("private-node-{i}.jsonl")),
                &manifest.context,
            )?;
            journal.append(event.clone())?;
        }
        println!(
            "{}",
            serde_json::json!({"recorded_in_three_durable_journals":args[3],"chain_check":"caller must verify actual canonical inclusion"})
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "freeze" {
        let out = Path::new(&args[1]);
        let manifest: Manifest = serde_json::from_slice(&read(&out.join("manifest.json"), 16384)?)?;
        let current = now()?;
        if current < manifest.closes_at_utc {
            return Err("declared cutoff has not occurred".into());
        }
        let book: FinalBook =
            serde_json::from_slice(&read(&out.join("accepted-book.json"), 32768)?)?;
        let wire: [[WireReceipt; 3]; 4] =
            serde_json::from_slice(&read(&out.join("acceptance-receipts.json"), 65536)?)?;
        let operators = manifest.operators()?;
        let receipts = wire
            .into_iter()
            .map(|r| AcceptedReceipt::from_wire(r, &operators, &manifest.context))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let rebuilt = FinalBook::from_receipts(&manifest, &receipts.try_into().ok().unwrap())?;
        if rebuilt != book {
            return Err("persisted accepted book differs from authenticated receipts".into());
        }
        let mut endorsements = Vec::new();
        for i in 0..3 {
            let secret: [u8; 32] = read(&out.join(format!("private-operator-{i}.bin")), 32)?
                .try_into()
                .map_err(|_| "operator key length")?;
            let key = SigningKey::from_bytes(&secret);
            let mut journal = Journal::open(
                &out.join(format!("private-node-{i}.jsonl")),
                &manifest.context,
            )?;
            journal.append(Event::Freeze)?;
            endorsements.push(Endorsement::issue(
                &mut journal,
                &book,
                &manifest,
                i,
                &key,
                current,
            )?);
        }
        let certificate = Certificate::combine(endorsements.try_into().ok().unwrap(), &manifest)?;
        write(
            &out.join("final-book-certificate.json"),
            &serde_json::to_vec_pretty(&certificate)?,
        )?;
        println!(
            "{}",
            serde_json::json!({"actual_wall_clock_cutoff_checked":true,"final_book_signatures":3,"receipt_quorums_reverified":4,"scope":"chain-admission operator certificate; not a public funding proof"})
        );
        return Ok(());
    }
    if args.len() != 7 || args[0] != "admit" {
        return Err("usage: regtest_admission admit ESCROW_WALLET PRIVATE_NOTES DEPOSIT_RUN BOOK_RUN NEW_LOCAL_DIRECTORY CUTOFF_UTC | freeze DIRECTORY".into());
    }
    let escrow = Path::new(&args[1]);
    let notes = Path::new(&args[2]);
    let deposit = Path::new(&args[3]);
    let book_run = Path::new(&args[4]);
    let out = PathBuf::from(&args[5]);
    let cutoff: u64 = args[6].parse()?;
    let local = root.join(".local").canonicalize()?;
    if out.exists()
        || !out
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(&local)
    {
        return Err("new ignored output required".into());
    }
    let book: Book = serde_json::from_slice(&read(&deposit.join("private-book.json"), 16384)?)?;
    let mut sorted = book.ids;
    sorted.sort();
    if book.active != [true; 4] || sorted != book.ids || sorted.windows(2).any(|i| i[0] == i[1]) {
        return Err("book order/active/duplicate mismatch".into());
    }
    let key: [u8; 32] = read(&escrow.join("private-spending-key.bin"), 32)?
        .try_into()
        .map_err(|_| "escrow key length")?;
    let fvk = FullViewingKey::from(
        &SpendingKey::from_bytes(key)
            .into_option()
            .ok_or("escrow key")?,
    );
    let openings: serde_json::Value = serde_json::from_slice(&read(notes, 1_000_000)?)?;
    if openings["scope"] != "isolated regtest wallet openings"
        || openings["genesis"] != GENESIS
        || openings["nu6_3_height"] != 2
    {
        return Err("snapshot scope mismatch".into());
    }
    let bytes = read(&deposit.join("regtest-transaction.bin"), 1_000_000)?;
    let mut cursor = std::io::Cursor::new(&bytes);
    let tx = Transaction::read(&mut cursor, BranchId::Nu6_3)?;
    if cursor.position() != bytes.len() as u64 {
        return Err("funding tx trailing bytes".into());
    }
    let tx = tx.into_data();
    let mapping: serde_json::Value =
        serde_json::from_slice(&read(&deposit.join("private-deposit-map.json"), 16384)?)?;
    let indices: [usize; 4] = serde_json::from_value(mapping["indices"].clone())?;
    let receivers: [[u8; 32]; 6] = serde_json::from_value(mapping["receiver_commitments"].clone())?;
    let claims: [WireClaim; 4] =
        serde_json::from_slice(&read(&deposit.join("private-claims.json"), 65536)?)?;
    fs::create_dir(&out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&out, fs::Permissions::from_mode(0o700))?;
    }
    let co = root.join(".deps/co-snarks/target-aes-native/release/co-noir");
    let crs = root.join(".deps/co-snarks/co-noir/co-noir-common/src/crs");
    checked(
        Command::new(&co)
            .args(["create-vk", "--circuit"])
            .arg(root.join("circuits/bid_input/target/darkcall_bid_input.json"))
            .arg("--crs")
            .arg(crs.join("bn254_g1.dat"))
            .args(["--hasher", "poseidon2", "--vk"])
            .arg(out.join("trusted-client-vk")),
        &out.join("client-vk.log"),
    )?;
    for i in 0..4 {
        checked(
            Command::new(&co)
                .arg("verify")
                .arg("--proof")
                .arg(book_run.join(format!("client{i}/proof_plaindriver")))
                .arg("--public-input")
                .arg(book_run.join(format!("client{i}/public_inputs_plaindriver")))
                .arg("--vk")
                .arg(out.join("trusted-client-vk"))
                .arg("--crs")
                .arg(crs.join("bn254_g2.dat"))
                .args(["--hasher", "poseidon2", "--has-zk"]),
            &out.join(format!("client-{i}.log")),
        )?;
        for party in 0..3 {
            checked(
                Command::new(root.join("target/release/darkcall-input-tool"))
                    .arg("check")
                    .arg(deposit.join("private-book.json"))
                    .arg(i.to_string())
                    .arg(party.to_string())
                    .arg(book_run.join(format!("bidder{i}.toml.{party}.shared")))
                    .arg(book_run.join(format!("client{i}/public_inputs_plaindriver"))),
                &out.join(format!("pair-{i}-{party}.log")),
            )?;
        }
    }
    let secrets: [[u8; 32]; 3] = std::array::from_fn(|_| {
        let mut b = [0; 32];
        OsRng.fill_bytes(&mut b);
        b
    });
    let keys = secrets.map(|s| SigningKey::from_bytes(&s));
    let manifest = Manifest {
        context: Context {
            format: 1,
            network_genesis: hex::decode(GENESIS)?.try_into().unwrap(),
            conditions_hash: book.condition_digest,
            hash_rule: HASH_RULE.into(),
            units: book.units,
            reserve: book.reserve,
            budget: book.budget,
            processing_fee: book.processing_fee,
        },
        operator_keys: keys.each_ref().map(|k| k.verifying_key().to_bytes()),
        closes_at_utc: cutoff,
        seller_receiver_commitment: receivers[4],
        operator_receiver_commitment: receivers[5],
    };
    let operators = manifest.operators()?;
    for (i, secret) in secrets.iter().enumerate() {
        write(&out.join(format!("private-operator-{i}.bin")), secret)?;
    }
    write(
        &out.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    let required = book
        .budget
        .checked_add(book.processing_fee)
        .ok_or("deposit overflow")?;
    let mut journals: Vec<_> = (0..3)
        .map(|i| {
            Journal::open(
                &out.join(format!("private-node-{i}.jsonl")),
                &manifest.context,
            )
        })
        .collect::<std::result::Result<_, _>>()?;
    let mut receipts = Vec::new();
    for i in 0..4 {
        if now()? >= cutoff {
            return Err("funding/client admission missed declared cutoff".into());
        }
        let claim = Claim::from_wire(claims[i].clone())?;
        let expected = ExpectedFunding {
            network_genesis: manifest.context.network_genesis,
            conditions: book.condition_digest,
            ticket: book.ids[i],
            required_value: required,
        };
        let verified = verify_output(&tx, indices[i], &fvk, &expected, &claim)?;
        let reg = verified.registration();
        let expected_pairs = book.pair_commitments[i]
            .each_ref()
            .map(|s| decimal_field_bytes(s))
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if reg.bid_commitment != decimal_field_bytes(&book.commitments[i])?
            || reg.receiver_commitment != receivers[i]
            || verified.pair_commitments().as_slice() != expected_pairs.as_slice()
        {
            return Err("claimed instructions differ from verified client input/receiver".into());
        }
        let owned = openings["notes"]
            .as_array()
            .ok_or("notes list")?
            .iter()
            .find(|n| n["cmx"] == serde_json::json!(reg.note))
            .ok_or("claimed funding not unspent in scanner snapshot")?;
        if owned["action_index"].as_u64() != Some(indices[i] as u64)
            || owned["amount"].as_u64() != Some(required)
            || owned["coinbase"] != false
        {
            return Err("actual funding shape mismatch".into());
        }
        let block: [u8; 32] = serde_json::from_value(owned["mined_block"].clone())?;
        let height: u32 = serde_json::from_value(owned["mined_height"].clone())?;
        let tip: u32 = serde_json::from_value(openings["tip_height"].clone())?;
        if tip < height.checked_add(1).ok_or("height overflow")? {
            return Err("funding requires two controlled confirmations".into());
        }
        let mut endorsed = Vec::new();
        for operator in 0..3 {
            let j = &mut journals[operator];
            j.append(Event::Observe {
                note: reg.note,
                value: required,
                block,
                height,
            })?;
            j.append(Event::Confirm {
                note: reg.note,
                block,
            })?;
            j.append(Event::Accept {
                ticket: reg.ticket,
                note: reg.note,
                bid_commitment: reg.bid_commitment,
                receiver_commitment: reg.receiver_commitment,
            })?;
            endorsed.push(darkcall_wallet_bridge::receipt::Receipt::issue(
                j,
                &verified,
                &operators,
                operator,
                &keys[operator],
            )?);
        }
        receipts.push(AcceptedReceipt::combine(
            endorsed.try_into().ok().unwrap(),
            &operators,
            &manifest.context,
        )?);
    }
    let receipts: [AcceptedReceipt; 4] = receipts.try_into().ok().unwrap();
    let accepted = FinalBook::from_receipts(&manifest, &receipts)?;
    if accepted.accepted_root != decimal_field_bytes(&book.root)? {
        return Err("accepted root differs from actual verified client book".into());
    }
    write(
        &out.join("acceptance-receipts.json"),
        &serde_json::to_vec_pretty(&receipts.each_ref().map(AcceptedReceipt::to_wire))?,
    )?;
    write(
        &out.join("accepted-book.json"),
        &serde_json::to_vec_pretty(&accepted)?,
    )?;
    let summary = serde_json::json!({"scope":"four actual scanned regtest deposit claims admitted; fixture tickets","funding_observations_fabricated":false,"client_zk_proofs_reverified":4,"delivered_share_pairs_reverified":12,"client_claim_signatures_verified":4,"operator_receipt_signatures":12,"accepted_root_matches":true,"receipt_locks_fsynced":true,"maximum_bids_read":false,"actual_wall_clock_admission_cutoff_checked":true,"canonical_chain_and_unspent_checks":"Python driver + SDK scanner; trusted local full-node RPC","final_book_signed":false,"native_settlement_broadcast":false});
    write(
        &out.join("summary.json"),
        &serde_json::to_vec_pretty(&summary)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
