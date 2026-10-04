//! Append-only, fsynced private ledger with an exclusive process lock. A failed
//! write poisons this handle; a truncated journal fails closed on restart.
//! This is local crash recovery, not authenticated operator receipts.
use darkcall_auction_core::{
    Terms,
    funding::{Ledger, Observation, Phase, Purpose, Registration},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

type Result<T> = std::result::Result<T, String>;
const MAX_JOURNAL_BYTES: u64 = 32 * 1024 * 1024;
fn fail(context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}: {error}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub format: u32,
    pub network_genesis: [u8; 32],
    pub conditions_hash: [u8; 32],
    pub hash_rule: String,
    pub units: u32,
    pub reserve: u64,
    pub budget: u64,
    pub processing_fee: u64,
}
impl Context {
    fn ledger(&self) -> Result<Ledger> {
        if self.format != 1 || self.hash_rule.is_empty() || self.hash_rule.len() > 128 {
            return Err("unsupported funding journal format/hash rule".into());
        }
        Ledger::new(Terms {
            units: self.units,
            reserve: self.reserve,
            budget: self.budget,
            processing_fee: self.processing_fee,
        })
        .map_err(|e| fail("journal terms", format!("{e:?}")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", deny_unknown_fields)]
pub enum Event {
    Observe {
        note: [u8; 32],
        value: u64,
        block: [u8; 32],
        height: u32,
    },
    Confirm {
        note: [u8; 32],
        block: [u8; 32],
    },
    Accept {
        ticket: [u8; 32],
        note: [u8; 32],
        bid_commitment: [u8; 32],
        receiver_commitment: [u8; 32],
    },
    Freeze,
    Invalidate {
        notes: BTreeSet<[u8; 32]>,
    },
    Prepare {
        effects: [u8; 32],
        cancellation: bool,
    },
    Broadcast {
        effects: [u8; 32],
    },
    Finalize {
        effects: [u8; 32],
    },
}
impl Event {
    fn apply(&self, ledger: &mut Ledger) -> Result<()> {
        let result = match self {
            Self::Observe {
                note,
                value,
                block,
                height,
            } => ledger
                .observe(Observation {
                    note: *note,
                    value: *value,
                    block: *block,
                    height: *height,
                })
                .map(|_| ()),
            Self::Confirm { note, block } => ledger.confirm(note, block).map(|_| ()),
            Self::Accept {
                ticket,
                note,
                bid_commitment,
                receiver_commitment,
            } => ledger.accept_validated(Registration {
                ticket: *ticket,
                note: *note,
                bid_commitment: *bid_commitment,
                receiver_commitment: *receiver_commitment,
            }),
            Self::Freeze => ledger.freeze().map(|_| ()),
            Self::Invalidate { notes } => ledger.invalidate_chain_notes(notes),
            Self::Prepare {
                effects,
                cancellation,
            } => ledger.prepare(
                *effects,
                if *cancellation {
                    Purpose::Cancellation
                } else {
                    Purpose::Settlement
                },
            ),
            Self::Broadcast { effects } => ledger.mark_broadcast(effects),
            Self::Finalize { effects } => ledger.mark_finalized(effects),
        };
        result.map_err(|e| fail("funding transition", format!("{e:?}")))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    sequence: u64,
    action: Event,
}

pub struct Journal {
    file: File,
    ledger: Ledger,
    sequence: u64,
    bytes: u64,
    poisoned: bool,
}
impl Journal {
    /// The parent directory must already be private and remain excluded from
    /// version control. Keep the returned handle alive to retain the lock.
    pub fn open(path: &Path, expected: &Context) -> Result<Self> {
        let mut ledger = expected.ledger()?;
        let parent = path.parent().ok_or("journal requires a parent directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(parent)
                .map_err(|e| fail("journal directory", e))?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("journal parent must have private permissions (0700)".into());
            }
        }
        let mut options = OpenOptions::new();
        options.read(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let (mut file, created) = match options.clone().create_new(true).open(path) {
            Ok(file) => (file, true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (
                options.open(path).map_err(|e| fail("open journal", e))?,
                false,
            ),
            Err(e) => return Err(fail("create journal", e)),
        };
        file.try_lock()
            .map_err(|e| fail("exclusive funding journal lock", e))?;
        let metadata = file.metadata().map_err(|e| fail("journal metadata", e))?;
        if !metadata.is_file() || metadata.len() > MAX_JOURNAL_BYTES {
            return Err("journal is not a bounded regular file".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.permissions().mode() & 0o077 != 0 || metadata.nlink() != 1 {
                return Err("journal must be private (0600) with one link".into());
            }
        }
        if created {
            let mut header =
                serde_json::to_vec(expected).map_err(|e| fail("header encoding", e))?;
            header.push(b'\n');
            file.write_all(&header)
                .map_err(|e| fail("initialize journal", e))?;
            file.sync_all().map_err(|e| fail("sync journal", e))?;
            File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(|e| fail("sync journal directory", e))?;
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|e| fail("rewind journal", e))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| fail("read journal", e))?;
        if bytes.is_empty() || !bytes.ends_with(b"\n") {
            return Err("empty/truncated funding journal; recovery required".into());
        }
        let mut lines = bytes[..bytes.len() - 1].split(|b| *b == b'\n');
        let actual: Context = serde_json::from_slice(lines.next().ok_or("missing context")?)
            .map_err(|e| fail("journal header", e))?;
        if actual != *expected {
            return Err(
                "funding journal belongs to different conditions/network/hash rules".into(),
            );
        }
        let mut sequence = 0u64;
        for line in lines {
            let record: Record =
                serde_json::from_slice(line).map_err(|e| fail("journal record", e))?;
            sequence = sequence.checked_add(1).ok_or("journal sequence overflow")?;
            if record.sequence != sequence {
                return Err("funding journal sequence gap or reordering".into());
            }
            record.action.apply(&mut ledger)?;
        }
        Ok(Self {
            file,
            ledger,
            sequence,
            bytes: bytes.len() as u64,
            poisoned: false,
        })
    }
    pub fn phase(&self) -> Phase {
        self.ledger.phase()
    }
    pub fn refund_queue(
        &self,
    ) -> Vec<([u8; 32], u64, darkcall_auction_core::funding::Disposition)> {
        self.ledger.refund_queue()
    }
    pub fn append(&mut self, action: Event) -> Result<()> {
        if self.poisoned {
            return Err("journal write previously failed; restart/recovery required".into());
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or("journal sequence overflow")?;
        let mut next = self.ledger.clone();
        action.apply(&mut next)?;
        let mut encoded = serde_json::to_vec(&Record { sequence, action })
            .map_err(|e| fail("journal encoding", e))?;
        encoded.push(b'\n');
        let length = self
            .bytes
            .checked_add(encoded.len() as u64)
            .filter(|n| *n <= MAX_JOURNAL_BYTES)
            .ok_or("funding journal limit reached; explicit migration required")?;
        // Never report a transition as successful before durable storage. A
        // crash after fsync is replayed; a partial failed append cannot resume.
        self.poisoned = true;
        self.file
            .write_all(&encoded)
            .map_err(|e| fail("append journal", e))?;
        self.file
            .sync_all()
            .map_err(|e| fail("sync funding transition", e))?;
        self.ledger = next;
        self.sequence = sequence;
        self.bytes = length;
        self.poisoned = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        path: std::path::PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "darkcall-journal-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            Self { path }
        }
        fn journal(&self) -> std::path::PathBuf {
            self.path.join("funding.jsonl")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }
    fn context() -> Context {
        Context {
            format: 1,
            network_genesis: [9; 32],
            conditions_hash: [10; 32],
            hash_rule: "bn254-poseidon2-packed-row-v1".into(),
            units: 3,
            reserve: 20,
            budget: 100,
            processing_fee: 15000,
        }
    }
    fn fund(j: &mut Journal) {
        j.append(Event::Observe {
            note: [1; 32],
            value: 15100,
            block: [8; 32],
            height: 10,
        })
        .unwrap();
        j.append(Event::Confirm {
            note: [1; 32],
            block: [8; 32],
        })
        .unwrap();
        j.append(Event::Accept {
            ticket: [2; 32],
            note: [1; 32],
            bid_commitment: [3; 32],
            receiver_commitment: [4; 32],
        })
        .unwrap();
    }
    #[test]
    fn restart_preserves_exclusive_reservation_and_rejects_changed_context() {
        let f = Fixture::new();
        let mut j = Journal::open(&f.journal(), &context()).unwrap();
        fund(&mut j);
        j.append(Event::Freeze).unwrap();
        j.append(Event::Prepare {
            effects: [5; 32],
            cancellation: false,
        })
        .unwrap();
        assert!(Journal::open(&f.journal(), &context()).is_err());
        drop(j);
        let mut j = Journal::open(&f.journal(), &context()).unwrap();
        assert_eq!(j.phase(), Phase::Prepared);
        let size = fs::metadata(f.journal()).unwrap().len();
        assert!(
            j.append(Event::Prepare {
                effects: [6; 32],
                cancellation: true
            })
            .is_err()
        );
        assert_eq!(fs::metadata(f.journal()).unwrap().len(), size);
        j.append(Event::Broadcast { effects: [5; 32] }).unwrap();
        drop(j);
        let mut changed = context();
        changed.network_genesis[0] ^= 1;
        assert!(Journal::open(&f.journal(), &changed).is_err());
        let mut j = Journal::open(&f.journal(), &context()).unwrap();
        assert_eq!(j.phase(), Phase::Broadcast);
        j.append(Event::Finalize { effects: [5; 32] }).unwrap();
        drop(j);
        assert_eq!(
            Journal::open(&f.journal(), &context()).unwrap().phase(),
            Phase::Finalized
        );
    }
    #[test]
    fn crash_tail_and_symlink_fail_closed() {
        let f = Fixture::new();
        drop(Journal::open(&f.journal(), &context()).unwrap());
        OpenOptions::new()
            .append(true)
            .open(f.journal())
            .unwrap()
            .write_all(b"{\"sequence\":1")
            .unwrap();
        assert!(Journal::open(&f.journal(), &context()).is_err());
        #[cfg(unix)]
        {
            let link = f.path.join("alias.jsonl");
            std::os::unix::fs::symlink(f.journal(), &link).unwrap();
            assert!(Journal::open(&link, &context()).is_err());
        }
    }
    #[test]
    fn restart_retains_wrong_amount_refunds_and_reorg_pause() {
        let f = Fixture::new();
        let mut j = Journal::open(&f.journal(), &context()).unwrap();
        fund(&mut j);
        j.append(Event::Observe {
            note: [6; 32],
            value: 7,
            block: [8; 32],
            height: 10,
        })
        .unwrap();
        j.append(Event::Freeze).unwrap();
        j.append(Event::Invalidate {
            notes: BTreeSet::from([[1; 32]]),
        })
        .unwrap();
        drop(j);
        let mut j = Journal::open(&f.journal(), &context()).unwrap();
        assert_eq!(j.phase(), Phase::ReorgPaused);
        assert_eq!(j.refund_queue()[0].1, 7);
        assert!(
            j.append(Event::Prepare {
                effects: [5; 32],
                cancellation: false
            })
            .is_err()
        );
    }
}
