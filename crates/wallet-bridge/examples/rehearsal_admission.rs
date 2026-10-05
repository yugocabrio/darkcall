//! OFFLINE browser rehearsal, never canonical funding or native authorization.
//! Existing client/share checks and receipt primitives; synthetic observations
//! use a disjoint network domain. No RPC, spending or canonical fixture writes.
use darkcall_input_tool::{context, decimal_field_bytes, field_bytes, receiver_commitment};
use darkcall_wallet_bridge::{
    claim::{Claim, DepositMemo, ExpectedFunding, Instructions, verify_decrypted_note},
    final_book::{Certificate, Endorsement, FinalBook, HASH_RULE, Manifest},
    journal::{Context, Event, Journal},
    receipt::{AcceptedReceipt, Receipt, WireReceipt},
};
use ed25519_dalek::SigningKey;
use group::GroupEncoding;
use orchard::{
    Note,
    keys::{FullViewingKey, Scope, SpendingKey},
    note::{NoteVersion, RandomSeed, Rho},
    value::NoteValue,
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    condition_digest: [u8; 32],
    units: u32,
    reserve: u64,
    budget: u64,
    closes_at_utc: u64,
}
#[derive(Deserialize, Serialize)]
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
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn random() -> [u8; 32] {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}
fn read(path: &Path) -> Result<Vec<u8>> {
    if !path
        .canonicalize()?
        .starts_with(root().join(".local").canonicalize()?)
    {
        return Err("ignored rehearsal path required".into());
    }
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let m = f.metadata()?;
    if !m.is_file() || m.len() > 65536 || m.permissions().mode() & 0o077 != 0 || m.nlink() != 1 {
        return Err("private rehearsal file type/size/permissions".into());
    }
    let mut b = Vec::new();
    f.read_to_end(&mut b)?;
    Ok(b)
}
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::File::open(path.parent().ok_or("parent")?)?.sync_all()?;
    Ok(())
}
fn checked(c: &mut Command, log: &Path) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(log)?;
    if !c
        .stdout(Stdio::from(f.try_clone()?))
        .stderr(Stdio::from(f))
        .status()?
        .success()
    {
        return Err("rehearsal proof/share gate rejected".into());
    }
    Ok(())
}
fn manifest(out: &Path) -> Result<Manifest> {
    let m: Manifest = serde_json::from_slice(&read(&out.join("manifest.json"))?)?;
    if m.context.network_genesis
        != <[u8; 32]>::from(Sha256::digest(b"DarkCall/browser-rehearsal/offline/v1"))
    {
        return Err("not an offline rehearsal authority".into());
    }
    m.operators()?;
    Ok(m)
}
fn keys(out: &Path) -> Result<[SigningKey; 3]> {
    let s: [[u8; 32]; 3] =
        serde_json::from_slice(&read(&out.join("private-application-keys.json"))?)?;
    Ok(s.map(|b| SigningKey::from_bytes(&b)))
}
fn init(out: &Path) -> Result<serde_json::Value> {
    let mut b = Vec::new();
    std::io::stdin().take(4097).read_to_end(&mut b)?;
    if b.len() > 4096 {
        return Err("setup bound".into());
    }
    let s: Setup = serde_json::from_slice(&b)?;
    if !(1..=3).contains(&s.units)
        || s.budget > 1000000
        || s.reserve > s.budget
        || s.closes_at_utc <= now()
    {
        return Err("rehearsal terms/cutoff".into());
    }
    let secrets = std::array::from_fn(|_| random());
    let keys = secrets.map(|b| SigningKey::from_bytes(&b));
    let m = Manifest {
        context: Context {
            format: 1,
            network_genesis: Sha256::digest(b"DarkCall/browser-rehearsal/offline/v1").into(),
            conditions_hash: s.condition_digest,
            hash_rule: HASH_RULE.into(),
            units: s.units,
            reserve: s.reserve,
            budget: s.budget,
            processing_fee: 15000,
        },
        operator_keys: keys.each_ref().map(|k| k.verifying_key().to_bytes()),
        closes_at_utc: s.closes_at_utc,
        seller_receiver_commitment: field_bytes(context(
            &s.condition_digest,
            s.units,
            s.reserve,
            s.budget,
            15000,
        )),
        operator_receiver_commitment: field_bytes(context(
            &s.condition_digest,
            s.units,
            s.reserve,
            s.budget,
            15001,
        )),
    };
    m.operators()?;
    write(&out.join("manifest.json"), &serde_json::to_vec(&m)?)?;
    write(
        &out.join("private-application-keys.json"),
        &serde_json::to_vec(&secrets)?,
    )?;
    for i in 0..3 {
        Journal::open(&out.join(format!("private-node-{i}.jsonl")), &m.context)?;
    }
    Ok(serde_json::json!({"initialized":true,"funding_scope":"offline-synthetic","authority":m}))
}
fn admit(out: &Path, bid: &Path, i: usize) -> Result<serde_json::Value> {
    if i >= 4 {
        return Err("ticket index".into());
    }
    let m = manifest(out)?;
    if now() >= m.closes_at_utc {
        return Err("bidding deadline passed".into());
    }
    let b: Book = serde_json::from_slice(&read(&bid.join("book.json"))?)?;
    if b.condition_digest != m.context.conditions_hash
        || b.units != m.context.units
        || b.reserve != m.context.reserve
        || b.budget != m.context.budget
        || b.processing_fee != m.context.processing_fee
        || b.active != std::array::from_fn(|n| n == i)
        || b.root.is_empty()
    {
        return Err("client auction context mismatch".into());
    }
    let bb = root().join(".deps/barretenberg-src/barretenberg/cpp/build-darkcall/bin/bb");
    let crs = root().join(".deps/co-snarks/co-noir/co-noir-common/src/crs");
    // Key prepared from the fixed circuit by the caller's trusted local setup,
    // never accepted from a browser bid or the client directory.
    checked(
        Command::new(bb)
            .args([
                "verify",
                "--scheme",
                "ultra_honk",
                "--verifier_target",
                "noir-recursive",
                "-c",
            ])
            .arg(&crs)
            .arg("-k")
            .arg(out.parent().ok_or("setup parent")?.join("trusted-vk/vk"))
            .arg("-p")
            .arg(bid.join("proof_plaindriver"))
            .arg("-i")
            .arg(bid.join("public_inputs_plaindriver")),
        &bid.join("admit-proof.log"),
    )?;
    for p in 0..3 {
        checked(
            Command::new(root().join("target/release/darkcall-input-tool"))
                .arg("check")
                .arg(bid.join("book.json"))
                .arg(i.to_string())
                .arg(p.to_string())
                .arg(bid.join(format!("bidder{i}.toml.{p}.shared")))
                .arg(bid.join("public_inputs_plaindriver")),
            &bid.join(format!("admit-pair-{p}.log")),
        )?;
    }
    // Fresh unmined V3 note: a synthetic funding observation, NOT chain funding.
    let sk = loop {
        if let Some(sk) = SpendingKey::from_bytes(random()).into_option() {
            break sk;
        }
    };
    let address = FullViewingKey::from(&sk).address_at(0u32, Scope::External);
    let rho = Rho::from_bytes(&[1; 32]).unwrap();
    let seed = loop {
        if let Some(seed) = RandomSeed::from_bytes(random(), &rho).into_option() {
            break seed;
        }
    };
    let note = Note::from_parts(
        address,
        NoteValue::from_raw(b.budget + 15000),
        rho,
        seed,
        NoteVersion::V3,
    )
    .unwrap();
    let client = SigningKey::from_bytes(&random());
    let memo = DepositMemo {
        network_genesis: m.context.network_genesis,
        conditions: b.condition_digest,
        ticket: b.ids[i],
        client_key: client.verifying_key().to_bytes(),
        nonce: random(),
    };
    let nonce = field_bytes(context(&random(), 1, 0, 0, 0));
    let receiver = receiver_commitment(
        context(
            &b.condition_digest,
            b.units,
            b.reserve,
            b.budget,
            b.processing_fee,
        ),
        &b.ids[i],
        0,
        &address.g_d().to_bytes(),
        &address.pk_d().inner().to_bytes(),
        &nonce,
    )?;
    let instructions = Instructions {
        bid_commitment: decimal_field_bytes(&b.commitments[i])?,
        pair_commitments: b.pair_commitments[i]
            .each_ref()
            .map(|s| decimal_field_bytes(s))
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "pair count")?,
        refund_receiver_commitment: receiver,
    };
    let id = orchard::note::ExtractedNoteCommitment::from(note.commitment()).to_bytes();
    let claim = Claim::sign(&memo, id, instructions, &client)?;
    let verified = verify_decrypted_note(
        &note,
        &memo.encode()?,
        &ExpectedFunding {
            network_genesis: m.context.network_genesis,
            conditions: b.condition_digest,
            ticket: b.ids[i],
            required_value: b.budget + 15000,
        },
        &claim,
    )?;
    let r = verified.registration();
    let operators = m.operators()?;
    let k = keys(out)?;
    if now() >= m.closes_at_utc {
        return Err("deadline passed during proof/share checks; no acceptance issued".into());
    }
    let mut receipts = Vec::new();
    for (p, key) in k.iter().enumerate() {
        let mut j = Journal::open(&out.join(format!("private-node-{p}.jsonl")), &m.context)?;
        // Fabricated fixture observation explicitly separated by offline domain.
        j.append(Event::Observe {
            note: r.note,
            value: b.budget + 15000,
            block: m.context.network_genesis,
            height: 1,
        })?;
        j.append(Event::Confirm {
            note: r.note,
            block: m.context.network_genesis,
        })?;
        j.append(Event::Accept {
            ticket: r.ticket,
            note: r.note,
            bid_commitment: r.bid_commitment,
            receiver_commitment: r.receiver_commitment,
        })?;
        receipts.push(Receipt::issue(&mut j, &verified, &operators, p, key)?);
    }
    let accepted = AcceptedReceipt::combine(
        receipts.try_into().map_err(|_| "receipt count")?,
        &operators,
        &m.context,
    )?;
    write(
        &out.join(format!("accepted-{i}.json")),
        &serde_json::to_vec(&accepted.to_wire())?,
    )?;
    Ok(
        serde_json::json!({"accepted":true,"client_input_proof_verified":true,"share_pairs_verified":3,"receipt_signatures_verified":3,
        "funding_scope":"offline-synthetic","chain_funding_verified":false,"native_settlement_started":false,
        "ticket":hex::encode(b.ids[i]),"commitment":b.commitments[i],"receipt":accepted.to_wire()}),
    )
}
fn close(out: &Path) -> Result<serde_json::Value> {
    let m = manifest(out)?;
    if now() < m.closes_at_utc {
        return Err("cutoff not reached".into());
    }
    let ops = m.operators()?;
    let k = keys(out)?;
    let receipts = (0..4)
        .map(|i| {
            let wire: [WireReceipt; 3] =
                serde_json::from_slice(&read(&out.join(format!("accepted-{i}.json")))?)?;
            Ok(AcceptedReceipt::from_wire(wire, &ops, &m.context)?)
        })
        .collect::<Result<Vec<_>>>()?;
    let book = FinalBook::from_receipts(
        &m,
        &receipts.try_into().map_err(|_| "four receipts required")?,
    )?;
    let mut e = Vec::new();
    for (p, key) in k.iter().enumerate() {
        let mut j = Journal::open(&out.join(format!("private-node-{p}.jsonl")), &m.context)?;
        j.append(Event::Freeze)?;
        e.push(Endorsement::issue(&mut j, &book, &m, p, key, now())?);
    }
    let c = Certificate::combine(e.try_into().map_err(|_| "three endorsements")?, &m)?;
    write(
        &out.join("final-certificate.json"),
        &serde_json::to_vec(&c)?,
    )?;
    Ok(
        serde_json::json!({"closed":true,"accepted_root":hex::encode(book.accepted_root),"certificate":c,"funding_scope":"offline-synthetic","fresh_clearing_or_settlement":false}),
    )
}

fn certified_book(out: &Path) -> Result<(Manifest, Certificate, Book)> {
    let m = manifest(out)?;
    let c: Certificate = serde_json::from_slice(&read(&out.join("final-certificate.json"))?)?;
    c.verify(&m)?;
    let b = &c.endorsements[0].book;
    let decimal = |bytes: &[u8; 32]| -> Result<String> {
        Ok(darkcall_input_tool::canonical_field(bytes)?.to_string())
    };
    let commitments = b
        .entries
        .each_ref()
        .map(|e| decimal(&e.bid_commitment))
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| "four leaves")?;
    let pairs = b
        .entries
        .iter()
        .map(|e| {
            e.pair_commitments
                .each_ref()
                .map(decimal)
                .into_iter()
                .collect::<Result<Vec<_>>>()?
                .try_into()
                .map_err(|_| "three pair commitments".into())
        })
        .collect::<Result<Vec<[String; 3]>>>()?
        .try_into()
        .map_err(|_| "four pairs")?;
    let book = Book {
        condition_digest: m.context.conditions_hash,
        ids: b.entries.each_ref().map(|e| e.ticket),
        active: [true; 4],
        units: m.context.units,
        reserve: m.context.reserve,
        budget: m.context.budget,
        processing_fee: m.context.processing_fee,
        commitments,
        pair_commitments: pairs,
        root: decimal(&b.accepted_root)?,
    };
    Ok((m, c, book))
}

fn prepare_clearing(out: &Path) -> Result<serde_json::Value> {
    let (m, _, book) = certified_book(out)?;
    if now() < m.closes_at_utc {
        return Err("cutoff not reached".into());
    }
    let mut input = Vec::new();
    std::io::stdin().take(4097).read_to_end(&mut input)?;
    if input.len() > 4096 {
        return Err("bounded client paths".into());
    }
    let paths: [PathBuf; 4] = serde_json::from_slice(&input)?;
    if paths.iter().any(|p| p.parent() != Some(out)) {
        return Err("own auction client paths only".into());
    }
    let target = out.join("clearing-book");
    fs::create_dir(&target)?;
    write(&target.join("book.json"), &serde_json::to_vec(&book)?)?;
    let mut used = [false; 4];
    for (old, path) in paths.iter().enumerate() {
        let source: Book = serde_json::from_slice(&read(&path.join("book.json"))?)?;
        if source.active != std::array::from_fn(|i| i == old)
            || source.condition_digest != book.condition_digest
            || source.units != book.units
            || source.reserve != book.reserve
            || source.budget != book.budget
            || source.processing_fee != book.processing_fee
        {
            return Err("client conditions differ from certified book".into());
        }
        let i = book
            .ids
            .iter()
            .position(|id| *id == source.ids[old])
            .ok_or("client ticket not certified")?;
        if used[i]
            || source.commitments[old] != book.commitments[i]
            || source.pair_commitments[old] != book.pair_commitments[i]
        {
            return Err("client commitment/pairs differ from certificate".into());
        }
        used[i] = true;
        let client = target.join(format!("client{i}"));
        fs::create_dir(&client)?;
        // Only public proof artifacts and existing share pairs. Never read
        // Prover.toml, salts, plaintext bids or aggregate their openings.
        for name in ["proof_plaindriver", "public_inputs_plaindriver"] {
            write(&client.join(name), &read(&path.join(name))?)?;
        }
        for p in 0..3 {
            checked(
                Command::new(root().join("target/release/darkcall-input-tool"))
                    .arg("reindex-share")
                    .arg(path.join(format!("bidder{old}.toml.{p}.shared")))
                    .arg(old.to_string())
                    .arg(i.to_string())
                    .arg(target.join(format!("bidder{i}.toml.{p}.shared"))),
                &target.join(format!("reindex-{i}-{p}.log")),
            )?;
            checked(
                Command::new(root().join("target/release/darkcall-input-tool"))
                    .arg("check")
                    .arg(target.join("book.json"))
                    .arg(i.to_string())
                    .arg(p.to_string())
                    .arg(target.join(format!("bidder{i}.toml.{p}.shared")))
                    .arg(client.join("public_inputs_plaindriver")),
                &target.join(format!("check-{i}-{p}.log")),
            )?;
        }
    }
    Ok(
        serde_json::json!({"prepared":true,"sorted_tickets":true,"pairs_checked":12,"maximum_bid_openings_read":false,"accepted_root":hex::encode(decimal_field_bytes(&book.root)?)}),
    )
}

fn check_clearing(out: &Path, run: &Path) -> Result<serde_json::Value> {
    let (m, c, book) = certified_book(out)?;
    let data = read(&run.join("public-input0"))?;
    if data.len() != 187 * 32 {
        return Err("fixed delivery_flags4 statement required".into());
    }
    let fields: Vec<[u8; 32]> = data
        .chunks_exact(32)
        .map(|b| b.try_into().unwrap())
        .collect();
    for field in &fields {
        darkcall_input_tool::canonical_field(field)?;
    }
    let scalar = |v: u64| {
        let mut b = [0; 32];
        b[24..].copy_from_slice(&v.to_be_bytes());
        b
    };
    let mut expected = Vec::new();
    expected.extend(book.condition_digest.map(|b| scalar(u64::from(b))));
    for id in book.ids {
        expected.extend(id.map(|b| scalar(u64::from(b))));
    }
    expected.extend([
        scalar(u64::from(book.units)),
        scalar(book.reserve),
        scalar(book.budget),
        scalar(book.processing_fee),
    ]);
    for value in &book.commitments {
        expected.push(decimal_field_bytes(value)?);
    }
    expected.push(c.endorsements[0].book.accepted_root);
    if fields[..169] != expected {
        return Err("clearing does not match exact certified book".into());
    }
    let small = |bytes: [u8; 32]| -> Result<u64> {
        if bytes[..24] != [0; 24] {
            return Err("clearing output exceeds u64".into());
        }
        Ok(u64::from_be_bytes(bytes[24..].try_into()?))
    };
    let price = small(fields[177])?;
    let sold = small(fields[178])?;
    if price < m.context.reserve || price > m.context.budget || sold > u64::from(m.context.units) {
        return Err("clearing result bounds".into());
    }
    Ok(
        serde_json::json!({"certified_book_matches":true,"certificate_signatures_verified":3,
        "accepted_root":hex::encode(c.endorsements[0].book.accepted_root),"price":price,"sold":sold,
        "funding_scope":"offline-synthetic","native_settlement":false,"proof_must_be_verified_separately":true}),
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let value = match args.first().map(String::as_str) {
        Some("init") if args.len() == 2 => init(Path::new(&args[1]))?,
        Some("admit") if args.len() == 4 => {
            admit(Path::new(&args[1]), Path::new(&args[2]), args[3].parse()?)?
        }
        Some("close") if args.len() == 2 => close(Path::new(&args[1]))?,
        Some("prepare-clearing") if args.len() == 2 => prepare_clearing(Path::new(&args[1]))?,
        Some("check-clearing") if args.len() == 3 => {
            check_clearing(Path::new(&args[1]), Path::new(&args[2]))?
        }
        _ => return Err("init OUT | admit OUT BID_DIR INDEX | close OUT".into()),
    };
    println!("{}", value);
    Ok(())
}
