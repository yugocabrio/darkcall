# DarkCall · ZECATHON

Private multi-unit auctions with uniform pricing and shielded ZEC settlement.

A seller offers K identical units. Each funded ticket bids for one unit.
Three REP3 nodes compute the result and its proof without reconstructing the
maximum-bid list. Native Ironwood settlement uses a 2-of-3 FROST escrow.

**The four-bid protocol has settled actual notes on an owned isolated regtest.**
Four client proofs and 12 delivered-share checks authenticate the inputs. Signed
acceptance receipts and a three-operator final-book certificate bind the accepted
root and recipients. REP3 computes a commitment-bound auction/settlement proof;
six Halo2 row proofs bind its private amounts and recipients to actual native
output commitments. The PCZT is approved before four threshold spends are signed.
The local full node accepts the transaction, and six SDK wallets confirm exact
receipts. Public-only replay verifies the native proof/signatures, six bindings,
normal C++ Barretenberg auction proof and certificate against an explicitly trusted
manifest. It does not prove chain inclusion by itself.

The historical funded run used **v1 refund delivery, whose transcript privacy
claim is withdrawn**: its deterministic mask commitments allowed a public
two-candidate dictionary attack. Current v2 delivery requires a **fresh uniform
mask and an independent private commitment blinder** for each refund/winner bit.
Actual REP3, independent C++ verification and offline native/FROST integration
pass with v2. A fresh funded v2 auction also completes new DKG deposits, native
settlement, two controlled-chain confirmations and six exact receipts. Public-testnet
NU7 settlement remains unfinished. Price-zero private winner delivery through
native proof/FROST and all six decryptions also passes with synthetic funding;
funded price-zero execution remains separate work. Local private per-ticket
status/refund receipts now verify against the fixed public proof, including the
zero-price native settlement context; transport and bidder identity remain
unfinished. NU7 unsigned six-output preparation passes, while its full proof/
signature gate is capacity-blocked and unverified. See [status](STATUS.md)
and [the reproduced attack and fix](docs/operator-fill-delivery.md) for exact scope.
Plaintext reference fixtures are correctness oracles, not privacy demonstrations.

## Project records

- [Protocol proposal](docs/protocol.md): rules, privacy model, custody, fees and settlement.
- [Current status](STATUS.md): demonstrated behavior, failed experiments and next action.
- [Task queue and schedule](TASKS.md): completion criteria and priorities.
- [Development instructions](AGENTS.md): how autonomous work continues.

## Demonstration target

Four funded bidders → confidential price computation → public proof →
tampered settlement rejected → shielded seller payment and refunds →
recipient wallets verify notes. The owned regtest gate is demonstrated;
the advancing public-testnet gate is a separate milestone.

The first target is four bids with the full flow. A 32-bid benchmark comes later.

## Trust model

- REP3 privacy: at most one semi-honest corrupt party; two colluding parties can recover bids.
- Custody: two of three signers can move funds; users cannot force a unilateral refund.
- All escrow operators have viewing capability and can see deposit and settlement metadata.
- Operators receive refunds and winning status. Public auction outcomes can imply bid information.
- v2 transcript masking assumes independent CSPRNG masks/blinders and Poseidon2 hiding; the circuit does not prove randomness quality.
- Fixed deposits do not prevent Sybil identities or seller shill bids.
- Prices, fills and timing can reveal information through inference.
- Item delivery is outside the payment protocol.

This project initially targets **ZEC on testnet**, not a ZSA exchange or mainnet custody.
The settlement proof is off-chain; Zcash consensus does not enforce auction rules.
All three MPC/custody participants currently run on one host. No malicious-MPC
security, production custody or network-metadata anonymity is claimed.

## License

Our original code is MIT licensed. Upstream components retain their own licenses.

## Current checks

```sh
cargo test --release --locked -p darkcall-auction-core -p darkcall-input-tool -p darkcall-wallet-bridge
cargo test --release --locked -p darkcall-output-binding --lib layout_tests
cargo clippy --locked --release -p darkcall-native-gate -p darkcall-output-binding -p darkcall-auction-core --all-targets -- -D warnings
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/protocol_lib
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/bid_input
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/auction4
.deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/auction4
```

See [toolchain pins](docs/toolchains.md) before compiling circuits and
[input commitments](docs/input-commitments.md) for the admitted-share statement.
The runner is `python3 -B scripts/run_mpc_gate.py`; it uses synthetic inputs on one
host. Standalone proofs bind to the supplied accepted root. The funded driver
additionally requires verified signed acceptance receipts, a final-book certificate
and actual SDK-scanned notes. Authority keys must be supplied explicitly.
Read [native gate](docs/native-gate.md) and [output binding experiment](docs/output-binding.md)
before interpreting their historical experiments; [current status](STATUS.md)
supersedes their older progress descriptions. The [canonical settlement encoding](docs/settlement-rows.md)
is bridged with BN254 Poseidon2 emulated in the native field. SHA is a measured
reference/fallback. Standalone synthetic transactions cannot be broadcast.

Native six-row gates require substantial memory and disk headroom. Run only one
heavy prover with the resource-guarded scripts. The original 33-column circuit
remains the default; explicit `--shared-native-binding` uses the separately compiled
18-column circuit. Versioned layout metadata selects a fixed circuit/key and
rejects unknown layouts or uploaded keys. Historical bundles without metadata
use the original key. The guards preserve checkpoints on failure.

The generated cross-language check, `target/release/examples/cross_noir UNUSED_LOCAL_NAME`,
compares the independent Rust oracle to pinned Noir ACVM constraints. Its 77 valid
and seven invalid cases are plaintext regressions, not MPC or ZK proof evidence.
