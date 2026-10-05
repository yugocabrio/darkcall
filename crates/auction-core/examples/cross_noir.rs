//! Actual Nargo/ACVM execution compared to the independent Rust sort oracle.
//! All fixtures are fabricated plaintext. This is NOT MPC or a privacy demo.
use darkcall_auction_core::{Bid, MAX_MONEY, Terms, clear};
use std::{fs, io::Write, path::Path, process::Command};

#[derive(Clone)]
struct Case {
    bids: [u64; 4],
    ids: [[u8; 32]; 4],
    active: [bool; 4],
    terms: Terms,
}
impl Case {
    fn expected(&self) -> Option<[u64; 10]> {
        let bids: Vec<_> = (0..4)
            .filter(|i| self.active[*i])
            .map(|i| Bid {
                ticket_id: self.ids[i],
                amount: self.bids[i],
            })
            .collect();
        let r = clear(self.terms, &bids).ok()?;
        let mut out = [
            r.price,
            r.sold as u64,
            r.seller_amount,
            r.operator_amount,
            r.network_fee,
            r.action_count as u64,
            0,
            0,
            0,
            0,
        ];
        for i in 0..4 {
            if self.active[i] {
                out[6 + i] = r
                    .refunds
                    .iter()
                    .find(|v| v.ticket_id == self.ids[i])
                    .unwrap()
                    .refunded;
            }
        }
        Some(out)
    }
    fn toml(&self, expected: [u64; 10], require_match: bool) -> String {
        // Numbers/arrays use only our generated u64/u8/bool values.
        format!(
            "bids = {:?}\nids = {:?}\nactive = {:?}\nunits = {}\nreserve = {}\nbudget = {}\nprocessing_fee = {}\nexpected = {:?}\nrequire_match = {}\n",
            self.bids,
            self.ids,
            self.active,
            self.terms.units,
            self.terms.reserve,
            self.terms.budget,
            self.terms.processing_fee,
            expected,
            require_match
        )
    }
}
fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1
        || args[0].is_empty()
        || !args[0]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("expected fresh ignored fixture name".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let directory = root.join(".local").canonicalize()?.join(&args[0]);
    fs::create_dir(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    }
    let nargo = root.join(".deps/noir-1.0.0-rc.2/nargo");
    let version = Command::new(&nargo).arg("--version").output()?;
    if !version.status.success()
        || !String::from_utf8(version.stdout)?.contains("nargo version = 1.0.0-rc.2\n")
    {
        return Err("wrong pinned Noir version".into());
    }
    let circuit = root.join("circuits/auction_reference4");
    let base = Case {
        bids: [90, 70, 60, 50],
        ids: [[128; 32], [127; 32], [255; 32], [0; 32]],
        active: [true; 4],
        terms: Terms {
            units: 3,
            reserve: 20,
            budget: 100,
            processing_fee: 15000,
        },
    };
    let mut cases = vec![base.clone()];
    for bids in [[0; 4], [20; 4], [100; 4], [19, 20, 21, 0], [1, 2, 3, 4]] {
        for reserve in [0, 20] {
            let mut c = base.clone();
            c.bids = bids;
            c.terms.reserve = reserve;
            cases.push(c);
        }
    }
    for active_bits in 0..16u8 {
        let mut c = base.clone();
        c.active = std::array::from_fn(|i| active_bits & (1 << i) != 0);
        for i in 0..4 {
            if !c.active[i] {
                c.bids[i] = 0;
            }
        }
        cases.push(c);
    }
    // Reproducible LCG is test-case generation only, never protocol randomness.
    let mut seed = 0x4441524b43414c4cu64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed
    };
    for _ in 0..48 {
        let mut c = base.clone();
        c.terms.units = (next() % 6 + 1) as u32;
        c.terms.reserve = next() % 101;
        c.active = std::array::from_fn(|_| next() & 8 != 0);
        for i in 0..4 {
            c.bids[i] = if c.active[i] { next() % 101 } else { 0 };
            for j in 0..32 {
                c.ids[i][j] = (next() >> 32) as u8;
            }
            c.ids[i][31] = i as u8; // Guaranteed distinct full-byte IDs.
        }
        cases.push(c);
    }
    let mut large = base.clone();
    large.terms.budget = MAX_MONEY / 4 - 15000;
    large.bids = [large.terms.budget; 4];
    cases.push(large);
    let mut suffix = base.clone();
    suffix.bids = [50; 4];
    suffix.ids = [[0; 32]; 4];
    for i in 0..4 {
        suffix.ids[i][31] = (3 - i) as u8;
    }
    cases.push(suffix);
    let valid = cases.len();
    for kind in 0..7 {
        let mut c = base.clone();
        match kind {
            0 => c.terms.units = 0,
            1 => c.terms.units = 33,
            2 => c.terms.reserve = 101,
            3 => c.bids[0] = 101,
            4 => c.ids[0] = c.ids[1],
            5 => c.terms.processing_fee = 0,
            _ => c.terms.budget = MAX_MONEY,
        }
        assert!(c.expected().is_none());
        cases.push(c);
    }
    let mut successes = 0;
    for (i, c) in cases.iter().enumerate() {
        let expected = c.expected();
        let path = directory.join(format!("case-{i}.toml"));
        write_new(
            &path,
            c.toml(expected.unwrap_or([0; 10]), expected.is_some())
                .as_bytes(),
        )?;
        let output = Command::new(&nargo)
            .args(["execute", "--program-dir"])
            .arg(&circuit)
            .arg("--prover-name")
            .arg(path.with_extension(""))
            .arg(format!("cross-reference-{i}"))
            .output()?;
        let log = directory.join(format!("case-{i}.log"));
        let mut bytes = output.stdout;
        bytes.extend(output.stderr);
        write_new(&log, &bytes)?;
        if output.status.success() != expected.is_some() {
            return Err(format!(
                "Noir/Rust validity or outcome differs at generated case {i}; inspect ignored log"
            )
            .into());
        }
        // Invalid gates must be an ACVM assertion/overflow, not CLI/parser failure.
        let diagnostic = String::from_utf8_lossy(&bytes);
        if expected.is_none()
            && !(diagnostic.contains("error: Failed constraint")
                && diagnostic.contains("Cannot satisfy constraint"))
        {
            return Err(format!("case {i} failed without the expected circuit assertion").into());
        }
        successes += 1;
    }
    // A deliberately wrong oracle must fail even with otherwise valid inputs.
    let mut wrong = base.expected().unwrap();
    wrong[0] += 1;
    let path = directory.join("wrong-oracle.toml");
    write_new(&path, base.toml(wrong, true).as_bytes())?;
    let output = Command::new(&nargo)
        .args(["execute", "--program-dir"])
        .arg(&circuit)
        .arg("--prover-name")
        .arg(path.with_extension(""))
        .output()?;
    let mut bytes = output.stdout;
    bytes.extend(output.stderr);
    write_new(&directory.join("wrong-oracle.log"), &bytes)?;
    let diagnostic = String::from_utf8_lossy(&bytes);
    if output.status.success()
        || !(diagnostic.contains("error: Failed constraint")
            && diagnostic.contains("Cannot satisfy constraint"))
    {
        return Err("wrong reference outcome was not rejected by ACVM".into());
    }
    let summary = format!(
        "{{\"scope\":\"plaintext generated Rust/Noir ACVM correctness regression, not MPC or a ZK proof\",\"valid_cases\":{valid},\"invalid_cases\":7,\"all_cases_checked\":{successes},\"wrong_oracle_rejected\":true,\"zk_proof_generated\":false,\"private_bid_inputs_read\":false}}\n"
    );
    write_new(&directory.join("summary.json"), summary.as_bytes())?;
    print!("{summary}");
    Ok(())
}
