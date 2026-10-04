# DarkCall status

Updated: 2026-10-04 UTC.

## Current stage

Implementation authorized. First specification/task-queue commit pushed to main.
Reference oracle and four-bid Noir calculation circuit implemented.

## Demonstrated

- SSH read and push access to the user-provided repository.
- Rust oracle: 7 tests passed, including 4,096 small-book invariant cases and
  all 24 input permutations of a four-bid book. Strict clippy passed.
- Noir rc.2 circuit: compilation and 8 tests passed, including bad budget,
  duplicates, fee shortfall, ties, empty books and insufficient demand.
- Noir rc.2 installed under ignored .deps/ after checking the release SHA256.
  Global Nargo beta.20 and Barretenberg nightly.20260424 were left unchanged.

## Not demonstrated

Collaborative proofs, input consistency, native threshold spends, testnet settlement,
native output binding, fixed transaction shape and recipient decryption.

## Next action

Finish the existing coNoir release build, then run the four separate synthetic
bidder inputs through REP3 witness generation and a ZK-enabled proof.
The circuit currently outputs price and sold count; it does not bind bid
commitments, funding attestations or native settlement.

## Running job

- coNoir build tool session: 44418.
- Command: cargo build --release --locked --manifest-path .deps/co-snarks/Cargo.toml --bin co-noir -j 2
- Log: .local/co-noir-build.log (ignored).
- Source: 467a2abbad940d40ed78f38d424cc21920988976.
- Do not launch another build before checking this session, the log and binary.

## Autonomous execution

The continuing task queue is TASKS.md. Same-chat heartbeat darkcall is ACTIVE,
every 15 minutes, through October 28, 2026 23:59 UTC. It resumes this chat's context.
Each run resumes from this file and the latest human instructions, records concrete
progress, runs appropriate checks, commits useful milestones and pushes to origin/main.
