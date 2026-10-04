# DarkCall status

Updated: 2026-10-04 20:13 UTC.

## Current stage

Implementation authorized. First specification/task-queue commit pushed to main.
Reference oracle and four-bid Noir calculation circuit implemented. Initial actual
REP3 witness/proof generation and standalone ZK verification passed.

## Demonstrated

- SSH read and push access to the user-provided repository.
- Rust oracle: 7 tests passed, including 4,096 small-book invariant cases and
  all 24 input permutations of a four-bid book. Strict clippy passed.
- Noir rc.2 circuit: compilation and 8 tests passed, including bad budget,
  duplicates, fee shortfall, ties, empty books and insufficient demand.
- Noir rc.2 installed under ignored .deps/ after checking the release SHA256.
  Global Nargo beta.20 and Barretenberg nightly.20260424 were left unchanged.
- coNoir 0.7.0 built from the pinned source, with the upstream lockfile unchanged.
- Four separate synthetic bidder input files were shared and merged for three
  localhost REP3 processes. They jointly generated the witness and ZK proof.
- Standalone verifier accepted the proof and rejected changing the public price
  from 50 to 49. All parties agreed on price 50 and sold count 3.
- Circuit domain: 8,192. Proof: 14,656 bytes. Initial observed witness stage
  approximately 0.6 seconds and proof stage approximately 3 seconds on this Mac.
  These are single-host observations, not independent-host benchmarks.
- Recovery and fresh runner reproduction are recorded in evidence/.

## Not demonstrated

Signed accepted-input/funding authentication, matching external Barretenberg
verification, native threshold spends, testnet settlement, native output binding, fixed native transaction
shape and recipient decryption. No deployed independent-host privacy validation.
Per-node peak RAM and network bytes have not been measured.

## Active implementation

Accepted commitments, client share-consistency proofs and node-side delivered-pair
checks are integrated. Noir shared library (8 tests), bid-input circuit (4
tests) and refactored auction circuit (8 tests) passed. Rust input tool's hash vector
test passed. Run .local/mpc-auction4-20261004T200620Z passed client proofs,
all 12 delivered-share checks, committed MPC proof generation and verification,
and rejected changed shares, price and accepted root. Follow-up run
.local/mpc-auction4-20261004T200736Z passed using a client verification key
derived from the compiled circuit. Public summary is committed under evidence/.
Strict workspace clippy passed; process 64607 completed and was consumed.

Upstream zcash-sign's 3 Ironwood v6 sighash regression tests passed at frost-tools
06c0dbdbb1bac9b99cd255e74ba2f387c295e660. This is validation of an upstream public
fixture, not our own threshold transaction or recipient decryption.

Jobs 54829, 52988 and 57960 completed with exit 0 and were consumed. Logs are
.local/zcash-sign-test.log, .local/plaindriver-build.log and .local/input-tool-test.log.

## Next action

Evaluate the matching external Barretenberg verifier and construct our own native
Ironwood/PCZT/FROST transaction. Follow with signed acceptance/funding metadata and
native output binding. The current circuit outputs price and sold count and binds
the supplied accepted root; it does not establish an authoritative funding root or
native settlement. Do not repeat passed gates without a change or unresolved concern.

## Running jobs

- Tool session 77458: matching Barretenberg source build, target bb, -j 2.
  Source under .deps/barretenberg-src; log .local/barretenberg-build.log.
  Inspect this job before starting another build.

## Completed jobs

- coNoir build tool session 44418 completed with exit 0 and was consumed.
- Command: cargo build --release --locked --manifest-path .deps/co-snarks/Cargo.toml --bin co-noir -j 2
- Log: .local/co-noir-build.log (ignored).
- Source: 467a2abbad940d40ed78f38d424cc21920988976.
- MPC runner sessions 62605 and 94537 ended and were consumed. No MPC processes
  remain from these runs. Do not poll consumed sessions.
- The initial verifier invocation failed because upstream's binary parser rejects
  a public-input filename with an extension. Identical bytes in an extensionless
  file verified successfully. The corrected runner reproduced all stages.

## Autonomous execution

The continuing task queue is TASKS.md. Same-chat heartbeat darkcall is ACTIVE,
every 15 minutes, through October 28, 2026 23:59 UTC. It resumes this chat's context.
Each run resumes from this file and the latest human instructions, records concrete
progress, runs appropriate checks, commits useful milestones and pushes to origin/main.
