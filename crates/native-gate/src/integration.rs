//! Auction/native proof/signing linkage. Standalone gates use synthetic funding;
//! the controlled regtest caller additionally verifies actual notes/receipts.
use darkcall_bn254_emulation::poseidon::{digest_words, pack_row, reference};
use darkcall_output_binding::poseidon_rows::{
    HASH_RULE, LayoutProver, RowLayout, binding_from_note, read_layout, row_public,
    verify_set_for_layout, write_layout,
};
use darkcall_settlement_types::{
    NativeBatchStatement, NativeContext, Row, link_four_bid_statements,
};
use orchard::Note;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

pub struct Integration {
    pub root: PathBuf,
    pub book: PathBuf,
    pub directory: PathBuf,
    pub context: NativeContext,
    pub delivery: Delivery,
    pub receiver_openings: Option<PathBuf>,
    pub funding_is_synthetic: bool,
    pub native_proof_source: Option<PathBuf>,
    pub layout: RowLayout,
}

pub struct Delivery {
    pub amounts: [u64; 6],
    pub price: u64,
    pub sold: u32,
    pub fee: u64,
    pub run: PathBuf,
    pub winners: Option<[bool; 4]>,
}

const REFUND_RULE: &str = "fresh-field-mask-refund-delivery-v2";
const FLAGS_RULE: &str = "fresh-field-mask-refund-and-winner-delivery-v2";

impl Delivery {
    /// Decode only an opening freshly re-derived from a verified transcript.
    /// The input tool authenticates context/root/tickets/mask commitments; these
    /// checks preserve the winner/refund relation at the native consumer too.
    fn from_opening(value: &serde_json::Value, run: PathBuf, flags: bool) -> Result<Self> {
        if value["delivery_rule"] != if flags { FLAGS_RULE } else { REFUND_RULE } {
            return Err("decoded delivery rule mismatch".into());
        }
        let amounts: [u64; 6] = serde_json::from_value(value["amounts"].clone())
            .map_err(|e| fail("private amount vector", e))?;
        let price = value["price"].as_u64().ok_or("delivery price")?;
        let sold = u32::try_from(value["sold"].as_u64().ok_or("delivery sold")?)
            .map_err(|e| fail("delivery sold range", e))?;
        let fee = value["network_fee"].as_u64().ok_or("delivery fee")?;
        if sold > 3
            || price > 100
            || fee != 30000
            || amounts[4] != price * u64::from(sold)
            || amounts[5] != 30000
            || amounts
                .iter()
                .map(|amount| u128::from(*amount))
                .sum::<u128>()
                + u128::from(fee)
                != 4 * 15100
        {
            return Err("delivered amounts/price/sold/fee mismatch".into());
        }
        let winners = if flags {
            let bits: [bool; 4] = serde_json::from_value(value["winners"].clone())
                .map_err(|e| fail("private winner vector", e))?;
            if bits.iter().filter(|bit| **bit).count() != sold as usize
                || bits
                    .iter()
                    .zip(&amounts)
                    .any(|(won, amount)| *amount != 100 - if *won { price } else { 0 })
            {
                return Err("delivered winner/refund/count mismatch".into());
            }
            Some(bits)
        } else {
            if price == 0
                || amounts[..4]
                    .iter()
                    .any(|amount| *amount != 100 && *amount != 100 - price)
            {
                return Err("refund-only delivery needs an unambiguous positive price".into());
            }
            None
        };
        Ok(Self {
            amounts,
            price,
            sold,
            fee,
            run,
            winners,
        })
    }
    pub fn rule(&self) -> &'static str {
        if self.winners.is_some() {
            FLAGS_RULE
        } else {
            REFUND_RULE
        }
    }
}

type Result<T> = std::result::Result<T, String>;
fn fail(context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}: {error}")
}

/// Create secret files with restricted permissions before writing any contents.
fn secret_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| fail("create private fixture", e))?;
    file.write_all(bytes)
        .map_err(|e| fail("write private fixture", e))?;
    file.sync_all()
        .map_err(|e| fail("sync private fixture", e))?;
    fs::File::open(path.parent().ok_or("private fixture parent")?)
        .and_then(|f| f.sync_all())
        .map_err(|e| fail("sync fixture directory", e))
}

fn checked(command: &mut Command, log: &Path) -> Result<()> {
    let file = fs::File::create(log).map_err(|e| fail("open command log", e))?;
    let status = command
        .stdout(Stdio::from(
            file.try_clone().map_err(|e| fail("log clone", e))?,
        ))
        .stderr(Stdio::from(file))
        .status()
        .map_err(|e| fail("run verifier", e))?;
    if !status.success() {
        return Err("application proof/link verification failed; inspect ignored logs".into());
    }
    Ok(())
}

impl Integration {
    /// Check delivery consumption alone, before native key/proof/signature work.
    pub fn delivery_gate_summary(&self) -> serde_json::Value {
        serde_json::json!({"scope":"native integration consumes verified blinded-mask refunds only; no native construction",
            "delivery_rule":self.delivery.rule(),"delivery_verified":true,
            "private_winner_flags_verified":self.delivery.winners.is_some(),
            "normal_cpp_delivery_verifier_passed":true,"maximum_bids_read":false,
            "price":self.delivery.price,"sold":self.delivery.sold,"native_transaction_built":false,
            "custody_signatures":false,"broadcast":false})
    }
    pub fn new(book: &Path, name: &str) -> Result<Self> {
        Self::new_with_layout(book, name, RowLayout::Working33)
    }
    pub fn new_with_layout(book: &Path, name: &str, layout: RowLayout) -> Result<Self> {
        Self::new_with_receipts(book, name, layout, false)
    }
    pub fn new_with_receipts(
        book: &Path,
        name: &str,
        layout: RowLayout,
        receipts: bool,
    ) -> Result<Self> {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("expected a simple unused local fixture name".into());
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();
        let local = root
            .join(".local")
            .canonicalize()
            .map_err(|e| fail("local directory", e))?;
        let book = book.canonicalize().map_err(|e| fail("book directory", e))?;
        if !book.starts_with(&local) {
            return Err("integration accepts ignored synthetic book fixtures only".into());
        }
        let directory = local.join(name);
        fs::create_dir(&directory).map_err(|e| fail("new fixture directory", e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .map_err(|e| fail("private directory permissions", e))?;
        }
        write_layout(&directory, layout).map_err(|e| fail("native layout checkpoint", e))?;
        checked(
            Command::new(root.join("target/release/darkcall-input-tool"))
                .args(["native-context"])
                .arg(book.join("book.json"))
                .arg(directory.join("context.json")),
            &directory.join("context-validation.log"),
        )?;
        let context = serde_json::from_slice(
            &fs::read(directory.join("context.json")).map_err(|e| fail("read context", e))?,
        )
        .map_err(|e| fail("parse context", e))?;
        // This gate deliberately constructs only the published four-bid fixture.
        let data: serde_json::Value = serde_json::from_slice(
            &fs::read(book.join("book.json")).map_err(|e| fail("read book", e))?,
        )
        .map_err(|e| fail("parse book", e))?;
        for (key, value) in [("units", 3), ("budget", 100), ("processing_fee", 15000)] {
            if data[key].as_u64() != Some(value) {
                return Err(
                    "offline integration requires the four-bid standard fixture terms".into(),
                );
            }
        }
        let flags = match data["reserve"].as_u64() {
            Some(0) => true,
            Some(20) => receipts,
            _ => return Err("native fixture reserve must be zero or 20".into()),
        };
        eprintln!("Committed private refund delivery started");
        let mut delivery_command = Command::new("python3");
        delivery_command
            .arg(root.join("scripts/run_delivery_gate.py"))
            .arg("--book-run")
            .arg(&book)
            .arg("--result-file")
            .arg(directory.join("delivery-result.json"));
        if flags {
            delivery_command.arg("--winner-flags");
        }
        checked(&mut delivery_command, &directory.join("delivery-gate.log"))?;
        let result: serde_json::Value = serde_json::from_slice(
            &fs::read(directory.join("delivery-result.json"))
                .map_err(|e| fail("delivery run read", e))?,
        )
        .map_err(|e| fail("delivery run parse", e))?;
        let run = PathBuf::from(result["run"].as_str().ok_or("delivery run path missing")?)
            .canonicalize()
            .map_err(|e| fail("delivery path", e))?;
        if !run.starts_with(&local)
            || result["hash_rule"] != if flags { FLAGS_RULE } else { REFUND_RULE }
        {
            return Err("unexpected delivery run/rule".into());
        }
        checked(
            Command::new("python3")
                .arg(root.join("scripts/verify_external.py"))
                .arg(&run)
                .args([
                    "--circuit",
                    if flags {
                        "delivery_flags4"
                    } else {
                        "delivery_private4"
                    },
                ]),
            &directory.join("delivery-external.log"),
        )?;
        // Consume a fresh opening of the exact independently verified statement,
        // not a detached JSON result previously emitted by the runner.
        let opening_path = directory.join("private-delivery.json");
        let mut open = Command::new(root.join("target/release/darkcall-input-tool"));
        open.arg(if flags {
            "flags-result"
        } else {
            "private-result"
        })
        .arg(book.join("book.json"))
        .arg(run.join("plan/masks.json"));
        if flags {
            open.arg(run.join("plan/winner-masks.json"));
        }
        open.arg(run.join("public-input0")).arg(&opening_path);
        checked(
            &mut open,
            &directory.join("delivery-opening-validation.log"),
        )?;
        if fs::metadata(&opening_path)
            .map_err(|e| fail("delivery metadata", e))?
            .len()
            > 4096
        {
            return Err("delivery data too large".into());
        }
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(&opening_path).map_err(|e| fail("private delivery read", e))?,
        )
        .map_err(|e| fail("private delivery parse", e))?;
        let delivery = Delivery::from_opening(&value, run, flags)?;
        eprintln!("Refund delivery verified without reconstructing maximum bids");
        let integration = Self {
            root,
            book,
            directory,
            context,
            delivery,
            receiver_openings: None,
            funding_is_synthetic: true,
            native_proof_source: None,
            layout,
        };
        if receipts {
            integration.export_ticket_memos()?;
        }
        Ok(integration)
    }

    fn export_ticket_memos(&self) -> Result<()> {
        if self.delivery.winners.is_none() {
            return Err("receipt memos require independently proved winner flags".into());
        }
        let public = self.directory.join("receipt-proof-public");
        fs::create_dir(&public).map_err(|e| fail("receipt public directory", e))?;
        for name in [
            "vk",
            "proof.0",
            "public-input0",
            "tampered-price",
            "tampered-root",
            "tampered-cipher",
            "tampered-winner",
        ] {
            secret_write(
                &public.join(name),
                &crate::regtest_spend::read_private(&self.delivery.run.join(name), 1048576)?,
            )?;
        }
        secret_write(
            &self.directory.join("receipt-book.json"),
            &fs::read(self.book.join("book.json")).map_err(|e| fail("receipt book", e))?,
        )?;
        for index in 0..4 {
            checked(
                Command::new(self.root.join("target/release/darkcall-input-tool"))
                    .arg("flags-ticket-memo-export")
                    .arg(self.directory.join("receipt-book.json"))
                    .arg(self.delivery.run.join("plan/masks.json"))
                    .arg(self.delivery.run.join("plan/winner-masks.json"))
                    .arg(public.join("public-input0"))
                    .arg(index.to_string())
                    .arg(self.directory.join(format!("ticket-{index}.memo"))),
                &self.directory.join(format!("receipt-export-{index}.log")),
            )?;
        }
        Ok(())
    }

    pub fn prove_and_check(
        &self,
        rows: &[Row; 6],
        notes: &[Note; 6],
        actual: [[u8; 32]; 6],
        effects_digest: [u8; 32],
    ) -> Result<ApprovedPlan> {
        let start = Instant::now();
        let statement = NativeBatchStatement {
            row_hashes: rows
                .each_ref()
                .map(|row| digest_words(&reference(&pack_row(&row.encode())))),
            action_indices: rows.each_ref().map(|row| row.action_index),
            cmxs: std::array::from_fn(|i| actual[rows[i].action_index as usize]),
            native_actions: actual,
        };
        link_four_bid_statements(&statement.row_hashes, &statement.bindings(), &actual)
            .map_err(|_| "native shape/action mismatch")?;
        secret_write(
            &self.directory.join("native-private-rows.json"),
            &serde_json::to_vec(&rows.each_ref().map(|row| row.encode().to_vec()))
                .map_err(|e| fail("row serialization", e))?,
        )?;
        fs::write(
            self.directory.join("native-public.json"),
            serde_json::to_vec_pretty(&statement)
                .map_err(|e| fail("statement serialization", e))?,
        )
        .map_err(|e| fail("write statement", e))?;
        fs::File::open(self.directory.join("native-public.json"))
            .and_then(|f| f.sync_all())
            .map_err(|e| fail("sync statement", e))?;
        fs::File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(|e| fail("sync row checkpoint directory", e))?;
        if let Some(source) = &self.native_proof_source
            && read_layout(source).map_err(|e| fail("source native layout", e))? != self.layout
        {
            return Err("native proof recovery layout differs from source".into());
        }
        let cached = self
            .native_proof_source
            .as_ref()
            .filter(|source| {
                (0..6).all(|i| source.join(format!("native-binding-proof-{i}")).exists())
            })
            .map(|source| {
                (0..6)
                    .map(|i| {
                        crate::regtest_spend::read_private(
                            &source.join(format!("native-binding-proof-{i}")),
                            65536,
                        )
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let cached_start = Instant::now();
        if let Some(proofs) = &cached {
            // Fully cached recovery needs only the original fixed VK, not a PK.
            // Invalid cached bytes still fail closed, never become fresh proofs.
            verify_set_for_layout(self.layout, &statement, proofs)
                .map_err(|e| fail("resumed complete native proof set", e))?;
        }
        let cached_set_verify_seconds = if cached.is_some() {
            cached_start.elapsed().as_secs_f64()
        } else {
            0.0
        };
        eprintln!("Integrated native row key preparation started");
        let key_start = Instant::now();
        let prover = if cached.is_some() {
            None
        } else {
            Some(LayoutProver::new(self.layout).map_err(|e| fail("native row key", e))?)
        };
        let key_seconds = key_start.elapsed().as_secs_f64();
        let mut times = Vec::new();
        let mut reused = 0;
        for i in 0..6 {
            eprintln!("Integrated native row {i} proof started");
            let t = Instant::now();
            let opening =
                binding_from_note(&rows[i], &notes[i]).map_err(|e| fail("native opening", e))?;
            let public = row_public(&statement, i).map_err(|e| fail("native statement", e))?;
            let previous = self
                .native_proof_source
                .as_ref()
                .map(|p| p.join(format!("native-binding-proof-{i}")));
            let proof = if let Some(proofs) = &cached {
                reused += 1;
                proofs[i].clone()
            } else if let Some(path) = previous.filter(|p| p.exists()) {
                let proof = crate::regtest_spend::read_private(&path, 65536)?;
                // Existing bytes are never accepted merely because a file is
                // present. Reverify under our trusted circuit and exact current
                // row/cmx/index; reject altered/partial proofs without fallback.
                prover
                    .as_ref()
                    .ok_or("missing native prover")?
                    .verify(&proof, &public)
                    .map_err(|e| fail("resumed native row proof", e))?;
                reused += 1;
                eprintln!("Integrated native row {i} existing proof reverified");
                proof
            } else {
                prover
                    .as_ref()
                    .ok_or("missing native prover")?
                    .prove(opening, &public)
                    .map_err(|e| fail("native row proof", e))?
            };
            let proof_path = self.directory.join(format!("native-binding-proof-{i}"));
            fs::write(&proof_path, proof).map_err(|e| fail("write row proof", e))?;
            fs::File::open(&proof_path)
                .and_then(|f| f.sync_all())
                .map_err(|e| fail("sync row proof", e))?;
            fs::File::open(&self.directory)
                .and_then(|f| f.sync_all())
                .map_err(|e| fail("sync row directory", e))?;
            times.push(t.elapsed().as_secs_f64());
        }
        // Release the large proving key before independent public verification.
        let proving_key_generated = prover.is_some();
        drop(prover);
        let summary = serde_json::json!({"proof_verified":true,"book_context_supplied":true,"hash_rule":HASH_RULE,"layout":self.layout.id(),"rows":6,"key_seconds":key_seconds,"row_proof_and_verify_seconds":times,"reused_native_row_proofs":reused,"seconds":start.elapsed().as_secs_f64(),"synthetic_inputs":self.funding_is_synthetic,"fills_from_plaintext_oracle_fixture":false,"private_fill_delivery_verified":true,"private_winner_flags_verified":self.delivery.winners.is_some(),"delivery_rule":self.delivery.rule(),"price":self.delivery.price,"sold":self.delivery.sold,"delivery_run":self.delivery.run.file_name().unwrap().to_string_lossy(),"funding_proven":false,"native_custody_signature_verified":false,"proving_key_generated":proving_key_generated,"cached_set_verify_seconds":cached_set_verify_seconds});
        fs::write(
            self.directory.join("native-proof-summary.json"),
            serde_json::to_vec_pretty(&summary).map_err(|e| fail("summary encode", e))?,
        )
        .map_err(|e| fail("summary write", e))?;
        eprintln!("Integrated committed-input MPC verification started");
        let mut command = Command::new("python3");
        command
            .arg(self.root.join("scripts/run_settlement_gate.py"))
            .arg("--book-run")
            .arg(&self.book)
            .arg("--native-run")
            .arg(&self.directory)
            .args(["--hash", "poseidon", "--result-file"])
            .arg(self.directory.join("mpc-result.json"));
        if let Some(path) = &self.receiver_openings {
            command.arg("--receiver-openings").arg(path);
        }
        checked(&mut command, &self.directory.join("mpc-gate.log"))?;
        let result: serde_json::Value = serde_json::from_slice(
            &fs::read(self.directory.join("mpc-result.json"))
                .map_err(|e| fail("MPC result read", e))?,
        )
        .map_err(|e| fail("MPC result parse", e))?;
        let mpc = PathBuf::from(result["run"].as_str().ok_or("MPC run path missing")?)
            .canonicalize()
            .map_err(|e| fail("MPC path", e))?;
        if !mpc.starts_with(self.root.join(".local")) || result["hash_rule"] != HASH_RULE {
            return Err("unexpected MPC result path/hash".into());
        }
        // C++ verifier derives its own trusted key and checks the source patches.
        checked(
            Command::new("python3")
                .arg(self.root.join("scripts/verify_external.py"))
                .arg(&mpc)
                .args(["--circuit", "settlement_poseidon4"]),
            &self.directory.join("external-verifier.log"),
        )?;
        checked(
            Command::new(self.root.join("target/release/darkcall-input-tool"))
                .arg("link-plan")
                .arg(self.book.join("book.json"))
                .arg(self.directory.join("native-public.json"))
                .arg(mpc.join("public-input0")),
            &self.directory.join("signer-plan-link.log"),
        )?;
        Ok(ApprovedPlan {
            statement,
            effects_digest,
            mpc,
        })
    }
}

/// Constructed only after native proof, client/share checks, auction proof and
/// trusted C++ verification have passed. Honest signer checks this exact effect.
pub struct ApprovedPlan {
    statement: NativeBatchStatement,
    effects_digest: [u8; 32],
    pub mpc: PathBuf,
}
impl ApprovedPlan {
    pub fn check(&self, actions: &[[u8; 32]], effects_digest: &[u8; 32]) -> Result<()> {
        if effects_digest != &self.effects_digest {
            return Err("transaction effects changed after application verification".into());
        }
        link_four_bid_statements(
            &self.statement.row_hashes,
            &self.statement.bindings(),
            actions,
        )
        .map_err(|_| "native outputs changed after application verification".into())
    }
}

/// Replay against transaction bytes and public proofs only. Input-note existence
/// and finality still require a node; fabricated anchors are valid offline only.
pub fn verify_public(directory: &Path, authority: Option<&Path>) -> Result<serde_json::Value> {
    use orchard::{
        bundle::BatchValidator,
        circuit::{OrchardCircuitVersion, VerifyingKey},
    };
    use rand::rngs::OsRng;
    use zcash_primitives::transaction::{Transaction, TxVersion};
    use zcash_protocol::consensus::BranchId;
    let started = Instant::now();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let directory = directory
        .canonicalize()
        .map_err(|e| fail("public fixture path", e))?;
    if !directory.starts_with(root.join(".local")) {
        return Err("only offline local verification is supported".into());
    }
    let layout = read_layout(&directory).map_err(|e| fail("public native layout", e))?;
    let read = |name: &str, limit: u64| -> Result<Vec<u8>> {
        let path = directory.join(name);
        if fs::metadata(&path)
            .map_err(|e| fail("public file metadata", e))?
            .len()
            > limit
        {
            return Err("public fixture file too large".into());
        }
        fs::read(path).map_err(|e| fail("public file read", e))
    };
    let result: serde_json::Value = serde_json::from_slice(&read("mpc-result.json", 4096)?)
        .map_err(|e| fail("MPC result parse", e))?;
    let mpc = PathBuf::from(result["run"].as_str().ok_or("MPC run path missing")?)
        .canonicalize()
        .map_err(|e| fail("MPC public path", e))?;
    if !mpc.starts_with(root.join(".local")) || result["hash_rule"] != HASH_RULE {
        return Err("unexpected public MPC path/hash".into());
    }
    let input_path = mpc.join("public-input0");
    if fs::metadata(&input_path)
        .map_err(|e| fail("MPC input metadata", e))?
        .len()
        != 232 * 32
    {
        return Err("wrong MPC statement size".into());
    }
    let encoded = fs::read(input_path).map_err(|e| fail("MPC public inputs", e))?;
    if let Some(authority) = authority {
        use darkcall_wallet_bridge::final_book::{Certificate, Manifest};
        if fs::metadata(authority)
            .map_err(|e| fail("trusted manifest metadata", e))?
            .len()
            > 32768
        {
            return Err("trusted manifest too large".into());
        }
        // The manifest is an explicit external trust input. It must NOT be
        // automatically discovered in an untrusted proof/receipt bundle.
        let manifest: Manifest = serde_json::from_slice(
            &fs::read(authority).map_err(|e| fail("trusted manifest read", e))?,
        )
        .map_err(|e| fail("trusted manifest parse", e))?;
        let certificate: Certificate =
            serde_json::from_slice(&read("final-book-certificate.json", 131072)?)
                .map_err(|e| fail("final-book certificate parse", e))?;
        certificate
            .verify_statement(&encoded, &manifest)
            .map_err(|e| fail("certified MPC acceptance", e))?;
    }
    let transaction_name = match (
        directory.join("synthetic-transaction.bin").exists(),
        directory.join("regtest-transaction.bin").exists(),
    ) {
        (true, false) => "synthetic-transaction.bin",
        (false, true) => "regtest-transaction.bin",
        _ => return Err("expected exactly one explicit transaction artifact".into()),
    };
    let bytes = read(transaction_name, 1000000)?;
    let mut cursor = std::io::Cursor::new(&bytes);
    let tx = Transaction::read(&mut cursor, BranchId::Nu6_3)
        .map_err(|e| fail("transaction parse", e))?;
    if cursor.position() != bytes.len() as u64
        || tx.version() != TxVersion::V6
        || tx.consensus_branch_id() != BranchId::Nu6_3
        || tx.transparent_bundle().is_some()
        || tx.sapling_bundle().is_some()
        || tx.orchard_bundle().is_some()
    {
        return Err("unexpected transaction format/pools/trailing data".into());
    }
    let statement: NativeBatchStatement =
        serde_json::from_slice(&read("native-public.json", 32768)?)
            .map_err(|e| fail("native statement parse", e))?;
    let bundle = tx
        .ironwood_bundle()
        .ok_or("missing Ironwood transaction bundle")?;
    let actual: Vec<_> = bundle
        .actions()
        .iter()
        .map(|a| a.cmx().to_bytes())
        .collect();
    link_four_bid_statements(&statement.row_hashes, &statement.bindings(), &actual)
        .map_err(|_| "public native output/action substitution")?;
    if statement.native_actions.as_slice() != actual {
        return Err("statement actions differ from serialized transaction".into());
    }
    let proofs = (0..6)
        .map(|i| read(&format!("native-binding-proof-{i}"), 65536))
        .collect::<Result<Vec<_>>>()?;
    // v6_signature_hash calls transparent_sig_digest(None) in this native-only
    // shape. That is exactly hash_transparent_txid_data(None), so the shielded
    // signature digest equals the raw txid. Transparent bundles are rejected
    // above; this shortcut must not be generalized to transparent spends.
    let digest: [u8; 32] = *tx.txid().as_ref();
    let vk = VerifyingKey::build(OrchardCircuitVersion::PostNu6_3);
    let mut batch = BatchValidator::new(&vk);
    batch
        .add_bundle(bundle, digest)
        .map_err(|e| fail("native proof/signature batch", e))?;
    if !batch.validate(OsRng) {
        return Err("native proof or spend/binding signatures invalid".into());
    }
    verify_set_for_layout(layout, &statement, &proofs)
        .map_err(|e| fail("public native row proofs", e))?;
    let label = format!(
        "external-replay-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| fail("clock", e))?
            .as_millis()
    );
    // Verify the exact bytes already authenticated above. External verification
    // must not reopen a potentially replaced public-input file from the source
    // and then check its proof against another statement. Snapshot PUBLIC data
    // only into a new private local directory; no witnesses/keys are copied.
    let snapshot = root.join(".local").join(&label);
    fs::create_dir(&snapshot).map_err(|e| fail("public verification snapshot", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&snapshot, fs::Permissions::from_mode(0o700))
            .map_err(|e| fail("snapshot permissions", e))?;
    }
    fs::write(snapshot.join("public-input0"), &encoded)
        .map_err(|e| fail("snapshot statement", e))?;
    for name in [
        "vk",
        "proof.0",
        "tampered-price",
        "tampered-plan",
        "tampered-row",
    ] {
        let path = mpc.join(name);
        if fs::metadata(&path)
            .map_err(|e| fail("MPC artifact metadata", e))?
            .len()
            > 1_048_576
        {
            return Err("MPC public artifact too large".into());
        }
        fs::write(
            snapshot.join(name),
            fs::read(path).map_err(|e| fail("MPC snapshot read", e))?,
        )
        .map_err(|e| fail("MPC snapshot write", e))?;
    }
    checked(
        Command::new("python3")
            .arg(root.join("scripts/verify_external.py"))
            .arg(&snapshot)
            .args(["--circuit", "settlement_poseidon4", "--output-label"])
            .arg(label),
        &directory.join("public-external-replay.log"),
    )?;
    // Recheck correspondence between the MPC row words and actual tx commitments.
    let mut hashes = [[0u32; 8]; 6];
    for (i, field) in encoded[184 * 32..].chunks_exact(32).enumerate() {
        if field[..28] != [0; 28] {
            return Err("digest word exceeds u32".into());
        }
        hashes[i / 8][i % 8] = u32::from_be_bytes(field[28..].try_into().unwrap());
    }
    link_four_bid_statements(&hashes, &statement.bindings(), &actual)
        .map_err(|_| "MPC/public transaction mismatch")?;
    Ok(
        serde_json::json!({"scope":"public-only transaction/application-proof replay; chain inclusion checked separately","private_openings_read":false,"native_proof_and_signatures_verified":true,"native_row_proofs_verified":true,"normal_cpp_auction_proof_verified":true,"actual_transaction_outputs_match":true,"hash_rule":HASH_RULE,"layout":layout.id(),"txid":tx.txid().to_string(),"transaction_bytes":bytes.len(),"chain_funding_or_finality_verified":false,"acceptance_or_recipient_receipts_authenticated":authority.is_some(),"acceptance_authentication_scope":"all-three operator attestation; not a public funding proof","seconds":started.elapsed().as_secs_f64()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_price_consumer_needs_explicit_verified_winners() {
        let value = serde_json::json!({"delivery_rule": FLAGS_RULE,
            "amounts":[100,100,100,100,0,30000],"price":0,"sold":3,
            "network_fee":30000,"winners":[true,true,true,false]});
        let opened = Delivery::from_opening(&value, PathBuf::new(), true).unwrap();
        assert_eq!(opened.winners, Some([true, true, true, false]));
        // Equal refunds do not establish which three tickets won.
        for winners in [
            serde_json::Value::Null,
            serde_json::json!([1, 1, 1, 0]),
            serde_json::json!([true, true, false, false]),
        ] {
            let mut changed = value.clone();
            changed["winners"] = winners;
            assert!(Delivery::from_opening(&changed, PathBuf::new(), true).is_err());
        }
        let mut refund_only = value.clone();
        refund_only["delivery_rule"] = REFUND_RULE.into();
        assert!(Delivery::from_opening(&refund_only, PathBuf::new(), false).is_err());
        let mut old_rule = value;
        old_rule["delivery_rule"] = "fresh-field-mask-refund-delivery-v1".into();
        assert!(Delivery::from_opening(&old_rule, PathBuf::new(), true).is_err());
    }

    #[test]
    fn native_consumer_checks_flag_refund_and_value_conservation() {
        let value = serde_json::json!({"delivery_rule": FLAGS_RULE,
            "amounts":[80,80,80,100,60,30000],"price":20,"sold":3,
            "network_fee":30000,"winners":[true,true,true,false]});
        assert!(Delivery::from_opening(&value, PathBuf::new(), true).is_ok());
        let mut changed = value.clone();
        changed["winners"] = serde_json::json!([false, true, true, true]);
        assert!(Delivery::from_opening(&changed, PathBuf::new(), true).is_err());
        for (field, replacement) in [("price", 101), ("sold", 4), ("network_fee", 1)] {
            let mut changed = value.clone();
            changed[field] = replacement.into();
            assert!(Delivery::from_opening(&changed, PathBuf::new(), true).is_err());
        }
        let mut changed = value;
        changed["amounts"][4] = 59.into();
        assert!(Delivery::from_opening(&changed, PathBuf::new(), true).is_err());
    }
    #[test]
    fn signer_rejects_changed_effects_outputs_mapping_and_shape() {
        let actions = std::array::from_fn(|i| [i as u8; 32]);
        let indices = [5, 2, 0, 4, 1, 3];
        let plan = ApprovedPlan {
            statement: NativeBatchStatement {
                row_hashes: std::array::from_fn(|i| [i as u32; 8]),
                action_indices: indices,
                cmxs: indices.map(|i| actions[i as usize]),
                native_actions: actions,
            },
            effects_digest: [7; 32],
            mpc: PathBuf::new(),
        };
        assert!(plan.check(&actions, &[7; 32]).is_ok());
        assert!(plan.check(&actions, &[8; 32]).is_err());
        let mut changed = actions;
        changed.swap(0, 5);
        assert!(plan.check(&changed, &[7; 32]).is_err());
        assert!(plan.check(&actions[..5], &[7; 32]).is_err());
    }
}
