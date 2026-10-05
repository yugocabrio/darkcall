//! Synthetic fixture creator and node-side validation of committed REP3 inputs.
//! No real bids, keys or shares should be placed in the Git repository.
use ark_bn254::Fr;
use ark_ff::PrimeField;
use bincode::Options;
use co_noir_types::{Rep3SharedInput, Rep3Type};
use darkcall_input_tool::{domain, hash, limbs};
use darkcall_settlement_types::{
    NativeBatchStatement, NativeContext, Role, Row, link_four_bid_statements,
};
use mpc_core::protocols::rep3;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, str::FromStr};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Serialize, Deserialize)]
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

impl Book {
    fn validate_four_active(&self) -> Result<()> {
        if self.active != [true; 4]
            || self
                .ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != 4
        {
            return Err("expected four active, distinct tickets".into());
        }
        for text in self.commitments.iter().chain(std::iter::once(&self.root)) {
            canonical_field(text)?;
        }
        if canonical_field(&self.root)? != self.root()? {
            return Err("accepted root does not match book".into());
        }
        Ok(())
    }

    fn native_context(&self) -> Result<NativeContext> {
        self.validate_four_active()?;
        let encoded = ark_ff::BigInteger::to_bytes_be(&canonical_field(&self.root)?.into_bigint());
        let mut accepted_root = [0u8; 32];
        accepted_root[32 - encoded.len()..].copy_from_slice(&encoded);
        Ok(NativeContext {
            conditions: self.condition_digest,
            accepted_root,
            ticket_ids: self.ids,
        })
    }
    fn context(&self) -> Fr {
        darkcall_input_tool::context(
            &self.condition_digest,
            self.units,
            self.reserve,
            self.budget,
            self.processing_fee,
        )
    }

    fn leaf(&self, id: &[u8; 32], bid: u64, salt: Fr) -> Fr {
        let mut fields = vec![domain(b"DarkCall/bid/v1"), self.context()];
        fields.extend(limbs(id));
        fields.extend([Fr::from(bid), salt]);
        hash(&fields)
    }

    fn pair(
        &self,
        bidder: usize,
        party: usize,
        b: rep3::Rep3PrimeFieldShare<Fr>,
        s: rep3::Rep3PrimeFieldShare<Fr>,
    ) -> Fr {
        let mut fields = vec![domain(b"DarkCall/share/v1"), self.context()];
        fields.extend(limbs(&self.ids[bidder]));
        fields.extend([Fr::from(party as u64), b.a, b.b, s.a, s.b]);
        hash(&fields)
    }

    fn root(&self) -> Result<Fr> {
        let fields = self.commitments.each_ref().map(|x| canonical_field(x));
        let fields = fields.into_iter().collect::<Result<Vec<_>>>()?;
        Ok(darkcall_input_tool::root(
            self.context(),
            self.active,
            fields.try_into().map_err(|_| "commitment count")?,
        ))
    }

    fn public_toml(&self) -> Result<String> {
        Ok(format!(
            "condition_digest = {}\nids = {}\nactive = {}\nunits = {}\nreserve = \"{}\"\nbudget = \"{}\"\nprocessing_fee = \"{}\"\ncommitments = {}\nroot = \"{}\"\n",
            serde_json::to_string(&self.condition_digest)?,
            serde_json::to_string(&self.ids)?,
            serde_json::to_string(&self.active)?,
            self.units,
            self.reserve,
            self.budget,
            self.processing_fee,
            serde_json::to_string(&self.commitments)?,
            self.root
        ))
    }

    fn client_public_toml(&self, bidder: usize) -> Result<String> {
        Ok(format!(
            "condition_digest = {}\nid = {}\nunits = {}\nreserve = \"{}\"\nbudget = \"{}\"\nprocessing_fee = \"{}\"\ncommitment = \"{}\"\npair_commitments = {}\n",
            serde_json::to_string(&self.condition_digest)?,
            serde_json::to_string(&self.ids[bidder])?,
            self.units,
            self.reserve,
            self.budget,
            self.processing_fee,
            self.commitments[bidder],
            serde_json::to_string(&self.pair_commitments[bidder])?
        ))
    }

    fn client_fields(&self, bidder: usize) -> Result<Vec<Fr>> {
        let mut fields = Vec::new();
        fields.extend(self.condition_digest.map(Fr::from));
        fields.extend(self.ids[bidder].map(Fr::from));
        fields.extend([
            Fr::from(self.units),
            Fr::from(self.reserve),
            Fr::from(self.budget),
            Fr::from(self.processing_fee),
        ]);
        fields.push(Fr::from_str(&self.commitments[bidder]).map_err(|_| "invalid commitment")?);
        for x in &self.pair_commitments[bidder] {
            fields.push(Fr::from_str(x).map_err(|_| "invalid pair commitment")?);
        }
        Ok(fields)
    }
}

fn canonical_field(text: &str) -> Result<Fr> {
    let field = Fr::from_str(text).map_err(|_| "invalid field")?;
    if field.to_string() != text {
        return Err("expected canonical decimal field".into());
    }
    Ok(field)
}

fn validate_rows(book: &Book, encoded: &[Vec<u8>]) -> Result<[Row; 6]> {
    let context = book.native_context()?;
    if encoded.len() != 6 {
        return Err("expected six native rows".into());
    }
    let mut used = [false; 6];
    let mut rows = Vec::new();
    for (i, bytes) in encoded.iter().enumerate() {
        let row = Row::decode(bytes).map_err(|_| "invalid settlement row encoding")?;
        let role = if i < 4 {
            Role::Refund
        } else if i == 4 {
            Role::Seller
        } else {
            Role::Operator
        };
        let ticket = if i < 4 { book.ids[i] } else { [0; 32] };
        if row.conditions != context.conditions
            || row.accepted_root != context.accepted_root
            || row.ticket != ticket
            || row.role != role
        {
            return Err("native row context/ticket/role mismatch".into());
        }
        let index = row.action_index as usize;
        if index >= 6 || used[index] {
            return Err("native action indices are not a permutation".into());
        }
        used[index] = true;
        rows.push(row);
    }
    rows.try_into().map_err(|_| "wrong row count".into())
}

// Offline glue only. It never reads or reconstructs any bidder's maximum bid.
// Authenticating recipient commitments against real receipts is separate work.
fn plan_fixture(
    book: &Book,
    row_path: &Path,
    out: &Path,
    receiver_openings: Option<&Path>,
) -> Result<()> {
    if fs::metadata(row_path)?.len() > 32768 {
        return Err("native row file too large".into());
    }
    let encoded: Vec<Vec<u8>> = serde_json::from_slice(&fs::read(row_path)?)?;
    let rows = validate_rows(book, &encoded)?;
    let mut rng = rand::thread_rng();
    let nonces: [Fr; 6] = if let Some(path) = receiver_openings {
        if fs::metadata(path)?.len() > 16384 {
            return Err("receiver openings too large".into());
        }
        let input: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
        let items = input
            .as_array()
            .ok_or("receiver openings must be a six-item list")?;
        if items.len() != 6 {
            return Err("receiver opening count".into());
        }
        items
            .iter()
            .map(|item| {
                let bytes: [u8; 32] = serde_json::from_value(item["nonce"].clone())?;
                darkcall_input_tool::canonical_field(&bytes).map_err(Into::into)
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| "receiver nonce count")?
    } else {
        std::array::from_fn(|_| <Fr as ark_ff::UniformRand>::rand(&mut rng))
    };
    let receivers: [String; 6] = std::array::from_fn(|i| {
        let row = &rows[i];
        let mut fields = vec![domain(b"DarkCall/receiver/v1"), book.context()];
        fields.extend(limbs(&row.ticket));
        fields.push(Fr::from(row.role as u32));
        fields.extend(limbs(&row.gd));
        fields.extend(limbs(&row.pkd));
        fields.push(nonces[i]);
        hash(&fields).to_string()
    });
    fs::create_dir(out)?;
    let gd = rows.each_ref().map(|row| row.gd);
    let pkd = rows.each_ref().map(|row| row.pkd);
    let salts = rows.each_ref().map(|row| row.salt);
    let indices = rows.each_ref().map(|row| row.action_index);
    let operator = format!(
        "gd = {}\npkd = {}\nreceiver_nonces = {}\nrow_salts = {}\n",
        serde_json::to_string(&gd)?,
        serde_json::to_string(&pkd)?,
        serde_json::to_string(&nonces.map(|x| x.to_string()))?,
        serde_json::to_string(&salts)?
    );
    let public = book
        .public_toml()?
        .lines()
        .filter(|line| !line.starts_with("active = "))
        .collect::<Vec<_>>()
        .join("\n")
        + &format!(
            "\nreceivers = {}\naction_indices = {}\n",
            serde_json::to_string(&receivers)?,
            serde_json::to_string(&indices)?
        );
    fs::write(out.join("operator.toml"), operator)?;
    fs::write(out.join("public.toml"), public)?;
    fs::write(
        out.join("expected-sha-row-hashes.json"),
        serde_json::to_vec_pretty(&rows.each_ref().map(|row| row.commitment()))?,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(out.join("operator.toml"), fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

// Proofs must already be verified by the caller. This checks their exact public
// context and row/action mapping; it makes no assertion about chain inclusion.
fn link_plan(book: &Book, native_path: &Path, mpc_path: &Path) -> Result<()> {
    book.validate_four_active()?;
    if fs::metadata(native_path)?.len() > 32768 || fs::metadata(mpc_path)?.len() != 232 * 32 {
        return Err("unexpected four-bid statement size".into());
    }
    let native: NativeBatchStatement = serde_json::from_slice(&fs::read(native_path)?)?;
    let mpc = fs::read(mpc_path)?;
    let mut expected = Vec::new();
    expected.extend(book.condition_digest.map(Fr::from));
    for id in book.ids {
        expected.extend(id.map(Fr::from));
    }
    expected.extend([
        Fr::from(book.units),
        Fr::from(book.reserve),
        Fr::from(book.budget),
        Fr::from(book.processing_fee),
    ]);
    for leaf in &book.commitments {
        expected.push(canonical_field(leaf)?);
    }
    expected.push(canonical_field(&book.root)?);
    for (encoded, field) in mpc.chunks_exact(32).zip(expected) {
        let bytes = ark_ff::BigInteger::to_bytes_be(&field.into_bigint());
        let mut canonical = [0u8; 32];
        canonical[32 - bytes.len()..].copy_from_slice(&bytes);
        if encoded != canonical {
            return Err("MPC public book does not match exact fixture".into());
        }
    }
    let mut hashes = [[0u32; 8]; 6];
    for (i, field) in mpc[184 * 32..].chunks_exact(32).enumerate() {
        if field[..28] != [0; 28] {
            return Err("row digest word exceeds u32".into());
        }
        hashes[i / 8][i % 8] = u32::from_be_bytes(field[28..].try_into().unwrap());
    }
    link_four_bid_statements(&hashes, &native.bindings(), &native.native_actions)
        .map_err(|_| "MPC/native row or action substitution")?;
    println!(
        "{}",
        serde_json::json!({"structural_link_passed":true,"proofs_must_be_verified_separately":true,
        "rows":6,"book_matches":true,"funding_or_chain_inclusion_proven":false})
    );
    Ok(())
}

fn book_fields(book: &Book) -> Result<Vec<Fr>> {
    book.validate_four_active()?;
    let mut fields = Vec::new();
    fields.extend(book.condition_digest.map(Fr::from));
    for id in book.ids {
        fields.extend(id.map(Fr::from));
    }
    fields.extend([
        Fr::from(book.units),
        Fr::from(book.reserve),
        Fr::from(book.budget),
        Fr::from(book.processing_fee),
    ]);
    for leaf in &book.commitments {
        fields.push(canonical_field(leaf)?);
    }
    fields.push(canonical_field(&book.root)?);
    Ok(fields)
}
fn encoded_field(value: Fr) -> [u8; 32] {
    let bytes = ark_ff::BigInteger::to_bytes_be(&value.into_bigint());
    let mut result = [0; 32];
    result[32 - bytes.len()..].copy_from_slice(&bytes);
    result
}
fn mask_commitment(book: &Book, i: usize, mask: Fr) -> Result<Fr> {
    let mut fields = vec![
        domain(b"DarkCall/fill-mask/v1"),
        book.context(),
        canonical_field(&book.root)?,
    ];
    fields.extend(limbs(&book.ids[i]));
    fields.push(mask);
    Ok(hash(&fields))
}
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)?;
    Ok(())
}
fn delivery_fixture(book: &Book, out: &Path) -> Result<()> {
    book.validate_four_active()?;
    fs::create_dir(out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(out, fs::Permissions::from_mode(0o700))?;
    }
    let mut rng = rand::thread_rng();
    let masks: [Fr; 4] = std::array::from_fn(|_| <Fr as ark_ff::UniformRand>::rand(&mut rng));
    let commitments = (0..4)
        .map(|i| mask_commitment(book, i, masks[i]).map(|f| f.to_string()))
        .collect::<Result<Vec<_>>>()?;
    write_private(
        &out.join("operator.toml"),
        format!(
            "output_masks = {}\n",
            serde_json::to_string(&masks.map(|m| m.to_string()))?
        )
        .as_bytes(),
    )?;
    write_private(
        &out.join("masks.json"),
        &serde_json::to_vec(&masks.map(|m| m.to_string()))?,
    )?;
    let public = book
        .public_toml()?
        .lines()
        .filter(|line| !line.starts_with("active = "))
        .collect::<Vec<_>>()
        .join("\n")
        + &format!(
            "\nmask_commitments = {}\n",
            serde_json::to_string(&commitments)?
        );
    fs::write(out.join("public.toml"), public)?;
    Ok(())
}
// Caller verifies the proof first. Only four operator masks and the public
// statement are read; maximum bids and their shares are never reconstructed.
fn delivery_result(book: &Book, masks_path: &Path, mpc_path: &Path, out: &Path) -> Result<()> {
    if fs::metadata(masks_path)?.len() > 4096 || fs::metadata(mpc_path)?.len() != 179 * 32 {
        return Err("unexpected delivery file size".into());
    }
    let masks: [String; 4] = serde_json::from_slice(&fs::read(masks_path)?)?;
    let masks = masks
        .map(|m| canonical_field(&m))
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    let encoded = fs::read(mpc_path)?;
    let mut fields = Vec::new();
    for bytes in encoded.chunks_exact(32) {
        let field = Fr::from_be_bytes_mod_order(bytes);
        if encoded_field(field) != bytes {
            return Err("noncanonical delivery field".into());
        }
        fields.push(field);
    }
    if fields[..169] != book_fields(book)? {
        return Err("delivery statement does not match book".into());
    }
    for i in 0..4 {
        if fields[169 + i] != mask_commitment(book, i, masks[i])? {
            return Err("delivery mask belongs to another auction/ticket".into());
        }
    }
    let small = |value: Fr| -> Result<u64> {
        let bytes = encoded_field(value);
        if bytes[..24] != [0; 24] {
            return Err("delivery amount exceeds u64".into());
        }
        Ok(u64::from_be_bytes(bytes[24..].try_into()?))
    };
    let price = small(fields[173])?;
    let sold = small(fields[174])?;
    if price > book.budget || sold > u64::from(book.units) {
        return Err("invalid delivery price/sold".into());
    }
    let refunds = (0..4)
        .map(|i| small(fields[175 + i] - masks[i]))
        .collect::<Result<Vec<_>>>()?;
    if refunds.iter().any(|v| *v > book.budget) {
        return Err("invalid delivered refund".into());
    }
    let fee = 30000u64;
    let operator_amount = book
        .processing_fee
        .checked_mul(4)
        .and_then(|v| v.checked_sub(fee))
        .ok_or("delivery fee shortfall")?;
    let seller_amount = price.checked_mul(sold).ok_or("seller amount overflow")?;
    let total = refunds
        .iter()
        .try_fold(0u64, |sum, v| sum.checked_add(*v))
        .and_then(|v| v.checked_add(seller_amount))
        .and_then(|v| v.checked_add(operator_amount))
        .and_then(|v| v.checked_add(fee))
        .ok_or("delivery amount overflow")?;
    if Some(total)
        != book
            .budget
            .checked_add(book.processing_fee)
            .and_then(|v| v.checked_mul(4))
    {
        return Err("delivered settlement is not conserved".into());
    }
    let mut amounts = refunds;
    amounts.extend([seller_amount, operator_amount]);
    write_private(
        out,
        &serde_json::to_vec_pretty(
            &serde_json::json!({"amounts":amounts,"price":price,"sold":sold,"network_fee":fee,"maximum_bids_read":false,"proof_must_be_verified_separately":true,"recipient_metadata_private_to_operators":true}),
        )?,
    )?;
    Ok(())
}

fn read_share(path: &Path) -> Result<Rep3SharedInput<Fr>> {
    if fs::metadata(path)?.len() > 16_384 {
        return Err("share file too large".into());
    }
    let bytes = fs::read(path)?;
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(16_384)
        .reject_trailing_bytes()
        .deserialize(&bytes)
        .map_err(|_| "invalid share encoding".into())
}

fn shared(map: &Rep3SharedInput<Fr>, key: &str) -> Result<rep3::Rep3PrimeFieldShare<Fr>> {
    match map.get(key) {
        Some(Rep3Type::Shared(value)) => Ok(*value),
        _ => Err("missing private share".into()),
    }
}

fn check(
    book: &Book,
    bidder: usize,
    party: usize,
    share_path: &Path,
    public_path: &Path,
) -> Result<()> {
    if bidder >= 4 || party >= 3 {
        return Err("invalid role".into());
    }
    let bytes = fs::read(public_path)?;
    let expected = book.client_fields(bidder)?;
    if bytes.len() != expected.len() * 32 {
        return Err("client statement length mismatch".into());
    }
    for (encoded, field) in bytes.chunks_exact(32).zip(expected) {
        let value = ark_ff::BigInteger::to_bytes_be(&field.into_bigint());
        let mut canonical = [0_u8; 32];
        canonical[32 - value.len()..].copy_from_slice(&value);
        if encoded != canonical {
            return Err("client proof statement mismatch".into());
        }
    }
    let map = read_share(share_path)?;
    if map.len() != 2 {
        return Err("unexpected share entries".into());
    }
    let b = shared(&map, &format!("bid{bidder}"))?;
    let s = shared(&map, &format!("salt{bidder}"))?;
    let target = Fr::from_str(&book.pair_commitments[bidder][party])
        .map_err(|_| "invalid pair commitment")?;
    if book.pair(bidder, party, b, s) != target {
        return Err("delivered share pair does not match proof".into());
    }
    Ok(())
}

fn fixture(out: &Path, values: [u64; 4]) -> Result<()> {
    if values.iter().any(|value| *value > 100) {
        return Err("synthetic bids exceed fixture budget".into());
    }
    fs::create_dir_all(out)?;
    let mut book = Book {
        condition_digest: [1; 32],
        ids: std::array::from_fn(|i| {
            let mut a = [0; 32];
            a[31] = i as u8;
            a
        }),
        active: [true; 4],
        units: 3,
        reserve: 20,
        budget: 100,
        processing_fee: 15_000,
        commitments: std::array::from_fn(|_| String::new()),
        pair_commitments: std::array::from_fn(|_| std::array::from_fn(|_| String::new())),
        root: String::new(),
    };
    let mut rng = rand::thread_rng();
    let mut private_client_files = Vec::new();
    for (bidder, bid) in values.into_iter().enumerate() {
        let salt = <Fr as ark_ff::UniformRand>::rand(&mut rng);
        let bids = rep3::share_field_element(Fr::from(bid), &mut rng);
        let salts = rep3::share_field_element(salt, &mut rng);
        book.commitments[bidder] = book.leaf(&book.ids[bidder], bid, salt).to_string();
        for party in 0..3 {
            book.pair_commitments[bidder][party] = book
                .pair(bidder, party, bids[party], salts[party])
                .to_string();
            let map = BTreeMap::from([
                (format!("bid{bidder}"), Rep3Type::Shared(bids[party])),
                (format!("salt{bidder}"), Rep3Type::Shared(salts[party])),
            ]);
            fs::write(
                out.join(format!("bidder{bidder}.toml.{party}.shared")),
                bincode::serialize(&map)?,
            )?;
        }
        private_client_files.push(format!(
            "bid = \"{bid}\"\nsalt = \"{salt}\"\nbid_shares = {}\nsalt_shares = {}\n",
            serde_json::to_string(&bids.map(|s| s.a.to_string()))?,
            serde_json::to_string(&salts.map(|s| s.a.to_string()))?
        ));
    }
    book.root = book.root()?.to_string();
    fs::write(out.join("book.json"), serde_json::to_vec_pretty(&book)?)?;
    fs::write(out.join("public.toml"), book.public_toml()?)?;
    for (bidder, private) in private_client_files.iter().enumerate() {
        let path = out.join(format!("client{bidder}"));
        fs::create_dir_all(&path)?;
        fs::write(
            path.join("Prover.toml"),
            private.to_owned() + &book.client_public_toml(bidder)?,
        )?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fixture") if args.len()==2 || args.len()==6 => {
            let values=if args.len()==6 {[args[2].parse()?,args[3].parse()?,args[4].parse()?,args[5].parse()?]} else {[90,70,60,50]};
            fixture(Path::new(&args[1]),values)?;
        },
        Some("check") if args.len()==6 => {
            let book:Book=serde_json::from_slice(&fs::read(&args[1])?)?;
            check(&book,args[2].parse()?,args[3].parse()?,Path::new(&args[4]),Path::new(&args[5]))?;
        },
        Some("tamper") if args.len()==4 => {
            let mut map=read_share(Path::new(&args[1]))?;
            let key=format!("bid{}",args[3].parse::<usize>()?);
            if let Some(Rep3Type::Shared(share))=map.get_mut(&key) {share.a += Fr::from(1);} else {return Err("missing bid".into());}
            fs::write(&args[2],bincode::serialize(&map)?)?;
        },
        Some("delivery-fixture") if args.len()==3 => {
            let book:Book=serde_json::from_slice(&fs::read(&args[1])?)?;
            delivery_fixture(&book,Path::new(&args[2]))?;
        },
        Some("delivery-result") if args.len()==5 => {
            let book:Book=serde_json::from_slice(&fs::read(&args[1])?)?;
            delivery_result(&book,Path::new(&args[2]),Path::new(&args[3]),Path::new(&args[4]))?;
        },
        Some("native-context") if args.len()==3 => {
            let book: Book = serde_json::from_slice(&fs::read(&args[1])?)?;
            fs::write(&args[2], serde_json::to_vec_pretty(&book.native_context()?)?)?;
        },
        Some("plan-fixture") if args.len()==4 || args.len()==5 => {
            let book: Book = serde_json::from_slice(&fs::read(&args[1])?)?;
            plan_fixture(&book, Path::new(&args[2]), Path::new(&args[3]), args.get(4).map(Path::new))?;
        },
        Some("link-plan") if args.len()==4 => {
            let book: Book = serde_json::from_slice(&fs::read(&args[1])?)?;
            link_plan(&book, Path::new(&args[2]), Path::new(&args[3]))?;
        },
        _ => return Err("usage: fixture OUT | check BOOK BIDDER PARTY SHARE CLIENT_PUBLIC_INPUT | tamper SHARE OUT BIDDER | native-context BOOK OUT | plan-fixture BOOK NATIVE_ROWS OUT | link-plan BOOK NATIVE_PUBLIC MPC_PUBLIC".into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_rejects_context_role_ticket_and_action_substitution() {
        let mut book = Book {
            condition_digest: [1; 32],
            ids: std::array::from_fn(|i| [i as u8; 32]),
            active: [true; 4],
            units: 3,
            reserve: 20,
            budget: 100,
            processing_fee: 15000,
            commitments: ["1", "2", "3", "4"].map(str::to_string),
            pair_commitments: std::array::from_fn(|_| std::array::from_fn(|_| "0".into())),
            root: String::new(),
        };
        book.root = book.root().unwrap().to_string();
        let context = book.native_context().unwrap();
        let rows: Vec<_> = (0..6)
            .map(|i| {
                Row {
                    conditions: context.conditions,
                    accepted_root: context.accepted_root,
                    ticket: if i < 4 { book.ids[i] } else { [0; 32] },
                    role: if i < 4 {
                        Role::Refund
                    } else if i == 4 {
                        Role::Seller
                    } else {
                        Role::Operator
                    },
                    action_index: [5, 2, 0, 4, 1, 3][i],
                    gd: [3; 32],
                    pkd: [4; 32],
                    amount: 50,
                    salt: [5; 32],
                }
                .encode()
                .to_vec()
            })
            .collect();
        assert!(validate_rows(&book, &rows).is_ok());
        // Conditions, root, ticket and role all come from the exact public book.
        for offset in [16, 48, 80, 112] {
            let mut changed = rows.clone();
            changed[0][offset] ^= 1;
            assert!(validate_rows(&book, &changed).is_err());
        }
        let mut changed = rows.clone();
        changed[1][116..120].copy_from_slice(&5u32.to_le_bytes());
        assert!(validate_rows(&book, &changed).is_err());
        let mut changed = rows.clone();
        changed[1][116..120].copy_from_slice(&6u32.to_le_bytes());
        assert!(validate_rows(&book, &changed).is_err());
        assert!(validate_rows(&book, &rows[..5]).is_err());
        book.root = "0".into();
        assert!(book.native_context().is_err());
        assert!(canonical_field("01").is_err());
    }

    #[test]
    fn private_delivery_rejects_wrong_masks_book_and_noncanonical_fields() {
        let mut book = Book {
            condition_digest: [1; 32],
            ids: std::array::from_fn(|i| [i as u8; 32]),
            active: [true; 4],
            units: 3,
            reserve: 20,
            budget: 100,
            processing_fee: 15000,
            commitments: ["1", "2", "3", "4"].map(str::to_string),
            pair_commitments: std::array::from_fn(|_| std::array::from_fn(|_| "0".into())),
            root: String::new(),
        };
        book.root = book.root().unwrap().to_string();
        let dir = std::env::temp_dir().join(format!(
            "darkcall-delivery-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let masks = [Fr::from(100), Fr::from(200), Fr::from(300), Fr::from(400)];
        let mut fields = book_fields(&book).unwrap();
        for (i, mask) in masks.iter().enumerate() {
            fields.push(mask_commitment(&book, i, *mask).unwrap());
        }
        fields.extend([Fr::from(50), Fr::from(3)]);
        for (mask, refund) in masks.iter().zip([50, 50, 50, 100]) {
            fields.push(*mask + Fr::from(refund));
        }
        let encoded = fields
            .into_iter()
            .flat_map(encoded_field)
            .collect::<Vec<_>>();
        let statement = dir.join("statement");
        fs::write(&statement, &encoded).unwrap();
        let pads = dir.join("pads");
        fs::write(
            &pads,
            serde_json::to_vec(&masks.map(|m| m.to_string())).unwrap(),
        )
        .unwrap();
        assert!(delivery_result(&book, &pads, &statement, &dir.join("valid")).is_ok());
        let mut wrong = masks;
        wrong[0] += Fr::from(1);
        fs::write(
            &pads,
            serde_json::to_vec(&wrong.map(|m| m.to_string())).unwrap(),
        )
        .unwrap();
        assert!(delivery_result(&book, &pads, &statement, &dir.join("bad-pad")).is_err());
        fs::write(
            &pads,
            serde_json::to_vec(&masks.map(|m| m.to_string())).unwrap(),
        )
        .unwrap();
        let mut invalid = encoded.clone();
        invalid[175 * 32..176 * 32].fill(255);
        fs::write(&statement, invalid).unwrap();
        assert!(delivery_result(&book, &pads, &statement, &dir.join("bad-field")).is_err());
        fs::write(&statement, &encoded).unwrap();
        book.condition_digest[0] ^= 1;
        book.root = book.root().unwrap().to_string();
        assert!(delivery_result(&book, &pads, &statement, &dir.join("bad-context")).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn matches_noir_standard_sponge() {
        // Hex vector is authoritative; convert it explicitly to avoid decimal transcription.
        let bytes = [
            0x00, 0x22, 0x47, 0xbe, 0x70, 0x14, 0xa5, 0x4d, 0x17, 0x34, 0x2a, 0x7e, 0xf6, 0x77,
            0xf5, 0x8d, 0x28, 0x87, 0x77, 0x80, 0xd2, 0x03, 0x86, 0x03, 0x96, 0x96, 0x7f, 0x5d,
            0x0a, 0x18, 0xd2, 0x59, 0xdb,
        ];
        let from_hex = Fr::from_be_bytes_mod_order(&bytes);
        assert_eq!(hash(&[1, 2, 3, 4, 5].map(Fr::from)), from_hex);
    }
}
