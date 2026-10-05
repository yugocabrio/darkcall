//! Offline auction/native proof/signing linkage. Synthetic funding and fills;
//! proof verification never authorizes a real chain spend in this fixture.
use darkcall_bn254_emulation::poseidon::{digest_words, pack_row, reference};
use darkcall_output_binding::poseidon_rows::{HASH_RULE, RowProver, binding_from_note, row_public};
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
}

pub struct Delivery {
    pub amounts: [u64; 6],
    pub price: u64,
    pub sold: u32,
    pub fee: u64,
    pub run: PathBuf,
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
        .map_err(|e| fail("write private fixture", e))
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
    pub fn new(book: &Path, name: &str) -> Result<Self> {
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
        for (key, value) in [
            ("units", 3),
            ("reserve", 20),
            ("budget", 100),
            ("processing_fee", 15000),
        ] {
            if data[key].as_u64() != Some(value) {
                return Err(
                    "offline integration requires the four-bid standard fixture terms".into(),
                );
            }
        }
        eprintln!("Committed private refund delivery started");
        checked(
            Command::new("python3")
                .arg(root.join("scripts/run_delivery_gate.py"))
                .arg("--book-run")
                .arg(&book)
                .arg("--result-file")
                .arg(directory.join("delivery-result.json")),
            &directory.join("delivery-gate.log"),
        )?;
        let result: serde_json::Value = serde_json::from_slice(
            &fs::read(directory.join("delivery-result.json"))
                .map_err(|e| fail("delivery run read", e))?,
        )
        .map_err(|e| fail("delivery run parse", e))?;
        let run = PathBuf::from(result["run"].as_str().ok_or("delivery run path missing")?)
            .canonicalize()
            .map_err(|e| fail("delivery path", e))?;
        if !run.starts_with(&local) || result["hash_rule"] != "fresh-field-mask-refund-delivery-v1"
        {
            return Err("unexpected delivery run/rule".into());
        }
        checked(
            Command::new("python3")
                .arg(root.join("scripts/verify_external.py"))
                .arg(&run)
                .args(["--circuit", "delivery4"]),
            &directory.join("delivery-external.log"),
        )?;
        if fs::metadata(run.join("delivered-amounts.json"))
            .map_err(|e| fail("delivery metadata", e))?
            .len()
            > 4096
        {
            return Err("delivery data too large".into());
        }
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(run.join("delivered-amounts.json"))
                .map_err(|e| fail("private delivery read", e))?,
        )
        .map_err(|e| fail("private delivery parse", e))?;
        let amounts = serde_json::from_value(value["amounts"].clone())
            .map_err(|e| fail("private amount vector", e))?;
        let delivery = Delivery {
            amounts,
            price: value["price"].as_u64().ok_or("delivery price")?,
            sold: u32::try_from(value["sold"].as_u64().ok_or("delivery sold")?)
                .map_err(|e| fail("delivery sold range", e))?,
            fee: value["network_fee"].as_u64().ok_or("delivery fee")?,
            run,
        };
        eprintln!("Refund delivery verified without reconstructing maximum bids");
        Ok(Self {
            root,
            book,
            directory,
            context,
            delivery,
            receiver_openings: None,
            funding_is_synthetic: true,
        })
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
        eprintln!("Integrated native row key generation started");
        let key_start = Instant::now();
        let prover = RowProver::new().map_err(|e| fail("native row key", e))?;
        let key_seconds = key_start.elapsed().as_secs_f64();
        let mut times = Vec::new();
        for i in 0..6 {
            eprintln!("Integrated native row {i} proof started");
            let t = Instant::now();
            let opening =
                binding_from_note(&rows[i], &notes[i]).map_err(|e| fail("native opening", e))?;
            let public = row_public(&statement, i).map_err(|e| fail("native statement", e))?;
            let proof = prover
                .prove(opening, &public)
                .map_err(|e| fail("native row proof", e))?;
            fs::write(
                self.directory.join(format!("native-binding-proof-{i}")),
                proof,
            )
            .map_err(|e| fail("write row proof", e))?;
            times.push(t.elapsed().as_secs_f64());
        }
        // Release the large proving key before independent public verification.
        drop(prover);
        let summary = serde_json::json!({"proof_verified":true,"book_context_supplied":true,"hash_rule":HASH_RULE,"rows":6,"key_seconds":key_seconds,"row_proof_and_verify_seconds":times,"seconds":start.elapsed().as_secs_f64(),"synthetic_inputs":self.funding_is_synthetic,"fills_from_plaintext_oracle_fixture":false,"private_fill_delivery_verified":true,"price":self.delivery.price,"sold":self.delivery.sold,"delivery_run":self.delivery.run.file_name().unwrap().to_string_lossy(),"funding_proven":false,"native_custody_signature_verified":false});
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
    use darkcall_output_binding::poseidon_rows::verify_set;
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
    verify_set(&statement, &proofs).map_err(|e| fail("public native row proofs", e))?;
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
        serde_json::json!({"scope":"public-only transaction/application-proof replay; chain inclusion checked separately","private_openings_read":false,"native_proof_and_signatures_verified":true,"native_row_proofs_verified":true,"normal_cpp_auction_proof_verified":true,"actual_transaction_outputs_match":true,"hash_rule":HASH_RULE,"txid":tx.txid().to_string(),"transaction_bytes":bytes.len(),"chain_funding_or_finality_verified":false,"acceptance_or_recipient_receipts_authenticated":authority.is_some(),"acceptance_authentication_scope":"all-three operator attestation; not a public funding proof","seconds":started.elapsed().as_secs_f64()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
