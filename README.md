# DarkCall · ZECATHON

Private multi-unit auctions with uniform pricing and shielded ZEC settlement.

A seller offers K identical units. Each funded ticket bids for one unit.
Three REP3 nodes are intended to compute the result and its proof without
reconstructing every bid. Native Ironwood settlement uses a 2-of-3 FROST escrow.

**Development began October 4, 2026. No end-to-end privacy or settlement claim
has been verified yet.** The integer reference oracle and initial four-bid Noir
circuit pass their tests. Four client input proofs, delivered-share checks and a
commitment-bound three-process REP3 auction proof now verify using the standalone
coNoir verifier; funding authentication and native settlement are unfinished.
Reference calculations are correctness oracles, not the live private auction.

## Project records

- [Protocol proposal](docs/protocol.md): rules, privacy model, custody, fees and settlement.
- [Current status](STATUS.md): demonstrated behavior, failed experiments and next action.
- [Task queue and schedule](TASKS.md): completion criteria and priorities.
- [Development instructions](AGENTS.md): how autonomous work continues.

## Target demonstration

Four funded bidders → confidential price computation → public proof →
tampered settlement rejected → testnet shielded seller payment and refunds →
recipient wallets verify notes.

The first target is four bids with the full flow. A 32-bid benchmark comes later.

## Trust model

- REP3 privacy: at most one semi-honest corrupt party; two colluding parties can recover bids.
- Custody: two of three signers can move funds; users cannot force a unilateral refund.
- All escrow operators have viewing capability and can see deposit and settlement metadata.
- Fixed deposits do not prevent Sybil identities or seller shill bids.
- Prices, fills and timing can reveal information through inference.
- Item delivery is outside the payment protocol.

This project initially targets **ZEC on testnet**, not a ZSA exchange or mainnet custody.
The target settlement proof is off-chain; Zcash consensus does not enforce auction rules.

## License

Our original code is MIT licensed. Upstream components retain their own licenses.

## Current checks

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/protocol_lib
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/bid_input
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/auction4
.deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/auction4
```

See [toolchain pins](docs/toolchains.md) before compiling circuits and
[input commitments](docs/input-commitments.md) for the admitted-share statement.
The runner is `python3 -B scripts/run_mpc_gate.py`; it uses synthetic inputs on one
host. The current proof binds to the supplied accepted root. Signed acceptance
receipts and funding checks are needed to establish which root is authoritative.
