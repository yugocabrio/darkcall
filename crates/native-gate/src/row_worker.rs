//! Local pipe-only warmed row prover. Same circuit/key/statements as the native
//! gate. No network listener, signing keys, MPC inputs or implicit approvals.
use crate::{Result, nu7_auction, regtest_spend::write_private};
use darkcall_output_binding::poseidon_rows::{LayoutProver, RowLayout, row_public};
use serde::Deserialize;
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
    time::Instant,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    source: PathBuf,
    name: String,
    rows: Vec<usize>,
}
impl Job {
    fn validate(&self) -> Result<()> {
        if self.name.is_empty()
            || self.name.len() > 50
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || self.rows.is_empty()
            || self.rows.len() > 6
            || self.rows.iter().any(|i| *i >= 6)
            || self
                .rows
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.rows.len()
        {
            return Err("bounded distinct worker rows and fresh simple name required".into());
        }
        Ok(())
    }
}

pub(crate) fn capacity(workers: usize) -> Result<()> {
    if ![1, 2, 3, 6].contains(&workers) {
        return Err("supported worker concurrency: 1/2/3/6".into());
    }
    if workers == 1 {
        return Ok(());
    }
    // No override: this development Mac must not run parallel heavy row proofs.
    if !cfg!(target_os = "linux") {
        return Err("parallel row proving requires a sufficiently provisioned Linux host".into());
    }
    let memory =
        std::fs::read_to_string("/proc/meminfo").map_err(|_| "parallel memory observation")?;
    let kib = |name: &str| -> Result<u64> {
        memory
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .ok_or("memory metric missing")?
            .split_whitespace()
            .next()
            .ok_or("memory metric missing")?
            .parse()
            .map_err(|_| "memory metric encoding".into())
    };
    let gib = 1024 * 1024;
    if kib("MemTotal:")? < (8 + 8 * workers as u64) * gib
        || kib("MemAvailable:")? < 8 * workers as u64 * gib
    {
        return Err("insufficient measured RAM for bounded parallel row proving".into());
    }
    Ok(())
}

fn emit(value: &serde_json::Value) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(value).map_err(|_| "worker summary encoding")?
    );
    std::io::stdout()
        .flush()
        .map_err(|_| "worker pipe closed".into())
}

pub fn run(workers: usize) -> Result<()> {
    capacity(workers)?;
    let started = Instant::now();
    let prover =
        LayoutProver::new(RowLayout::Shared18).map_err(|_| "worker fixed key preparation")?;
    emit(
        &serde_json::json!({"ready":true,"layout":RowLayout::Shared18.id(),"workers":workers,
        "cold_key_seconds":started.elapsed().as_secs_f64(),"proving_key_generated":true}),
    )?;
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    loop {
        let mut bytes = Vec::new();
        let count = input
            .by_ref()
            .take(4097)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| "worker bounded pipe read")?;
        if count == 0 {
            break;
        }
        if count > 4096 || bytes.last() != Some(&b'\n') {
            return Err("worker request exceeds bounded JSON line".into());
        }
        let job: Job = serde_json::from_slice(&bytes).map_err(|_| "strict worker job schema")?;
        job.validate()?;
        let (statement, bindings) = nu7_auction::worker_bindings(&job.source)?;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let output = root.join(".local").join(&job.name);
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&output)
            .map_err(|_| "preserve existing worker job")?;
        let job_start = Instant::now();
        let mut results = Vec::new();
        for batch in job.rows.chunks(workers) {
            let completed = std::thread::scope(|scope| {
                let handles = batch.iter().map(|i| {
                    let opening = bindings[*i].clone();
                    let public = row_public(&statement, *i).map_err(|_| "worker public statement")?;
                    let prover = &prover;
                    let path = output.join(format!("native-binding-proof-{i}"));
                    Ok(scope.spawn(move || -> Result<serde_json::Value> {
                        let row_start = Instant::now();
                        let proof = prover.prove(opening, &public).map_err(|_| "worker fixed proof/verification failed")?;
                        write_private(&path, &proof)?;
                        Ok(serde_json::json!({"row":i,"proof_bytes":proof.len(),"proof_and_verify_seconds":row_start.elapsed().as_secs_f64()}))
                    }))
                }).collect::<Result<Vec<_>>>()?;
                handles
                    .into_iter()
                    .map(|handle| handle.join().map_err(|_| "worker thread failed")?)
                    .collect::<Result<Vec<_>>>()
            })?;
            results.extend(completed);
        }
        emit(
            &serde_json::json!({"completed":true,"job":job.name,"rows":results,"workers":workers,
            "proving_key_generated":false,"same_fixed_verifier":true,"seconds":job_start.elapsed().as_secs_f64(),
            "native_signatures_or_auction_approval_created":false}),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn job_and_capacity_fail_closed_before_key_allocation() {
        for rows in [vec![], vec![0, 0], vec![6]] {
            assert!(
                Job {
                    source: PathBuf::new(),
                    name: "safe".into(),
                    rows
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            Job {
                source: PathBuf::new(),
                name: "../unsafe".into(),
                rows: vec![0]
            }
            .validate()
            .is_err()
        );
        assert!(
            serde_json::from_str::<Job>(r#"{"source":"x","name":"a","rows":[0],"vk":"untrusted"}"#)
                .is_err()
        );
        assert!(capacity(0).is_err());
        if !cfg!(target_os = "linux") {
            assert!(capacity(2).is_err());
            assert!(capacity(6).is_err());
        }
    }
}
