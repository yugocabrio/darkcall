# DarkCall status

Updated: 2026-10-04 20:34 UTC.

## Current stage

Implementation authorized. Specification, oracle and committed-input MPC milestone
pushed to main (d3c63f0). Offline native signing/transaction and one-row native
binding experiments passed; their coherent milestone is being prepared for push.

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
- Four client ZK input proofs against the trusted circuit key and all twelve
  delivered REP3 pair checks passed. Changed shares, public price and accepted
  root were rejected. See evidence/2026-10-04-committed-input-gate.json.
- Our offline v6 Ironwood/PCZT/FROST gate passed proof generation, 2-of-3
  signatures, full transaction extraction/verification and 6 recipient decryptions.
  4 synthetic inputs, 6 outputs/actions, standard fee 30,000, 21,790 transaction
  bytes. A second proved case includes zero seller/operator rows with the same
  shape and decryption of all 6 outputs. Neither fabricated transaction is broadcastable.
- One-row Halo2 proof binds a private amount/expanded recipient to a native
  Sinsemilla cmx: K=12, 4,928 proof bytes, 9.53 seconds observed total. Changed
  row hash/cmx, amount and recipient were rejected. This prototype uses Pallas
  Poseidon, assumes classical binding, and has not been connected to the auction
  plan. ZIP 2005 rseed derivation and ciphertext are not constrained in it.
- Latest strict workspace clippy passed (session 52787 consumed).

## Not demonstrated

Signed accepted-input/funding authentication, matching external Barretenberg
verification, testnet-funded threshold spends, broadcast/finality, and binding
the actual auction settlement plan to native outputs. Other supported native
input counts remain untested. No deployed independent-host privacy validation.
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

Connect the auction plan to native row commitments with a hash constrained in
both circuit families. Evaluate the matching C++ verifier while it builds.
Signed acceptance/funding metadata and testnet wallet integration also remain.
The auction proof currently outputs price/sold count and binds the supplied root;
it does not establish an authoritative funding root or native settlement.
Do not repeat passed gates without a change or unresolved concern.

## Running jobs

- Tool session 93437: matching Barretenberg full source build, target bb, -j 2.
  Source .deps/barretenberg-src, commit 87aa5283d5b0cab6632ed49fdd25f6548d2ba91f.
  Log .local/barretenberg-full-build.log, BB_LITE=OFF, AVM=OFF.
  Initial session 77458 failed under Apple Clang 15 on template deduction.
  One-line compatibility patch: patches/barretenberg-apple-clang15.patch.
  Retry session 25775 then failed at linking because BB_LITE excludes IPC symbols
  still used by api_msgpack. Both failed sessions were consumed. The full build
  changes that option without another source patch; inspect before duplicate work.

Completed and consumed: native proof 91827, native tests 14189, binding proof
74999, zero-row proof 42504, strict clippy 52787. Summaries are in evidence/.
Workspace tests 59783 passed (7 oracle, 1 input hash, 2 native tests) and were
consumed. Binding proof validation is an executable experiment, not a unit test.

Fast native construction run succeeded: 4 synthetic Ironwood spends, 6 requested
outputs under standard ZIP 317 fee 30,000, 2-of-3 FROST signatures and changed
sighash rejection. Two native tests passed, including zero seller/operator rows
that retain all 6 action/output slots and decrypt the zero-valued output notes.
Pins follow frost-tools' lockfile: native registry RedDSA 0.5.1 and rerandomized
FROST 2.1.0, rather than newly resolved 0.5.2/2.2.0.

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
