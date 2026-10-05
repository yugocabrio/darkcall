# DarkCall · ZECATHON

Private multi-unit auctions with uniform pricing and shielded ZEC settlement.

A seller offers K identical units. Each funded ticket bids for one unit.
Three REP3 nodes compute the result and its proof without reconstructing the
maximum-bid list. Native Ironwood settlement uses a 2-of-3 FROST escrow.

**One actual funded NU7 run now connects deposits, certified bids, four encrypted
receipts and six output bindings to the SAME DKG-authorized transaction.**
Zebra accepts it, two controlled confirmations and six exact recipient scans pass,
and all three durable journals finalize that transaction. Public native/Halo2/C++
verification and each bidder's own memo client use the same tx47019e02…a211938.

```sh
./demo.sh
./verify.sh --funded demo/artifacts/canonical-nu7-funded-v1 \
  --authority demo/authority/canonical-nu7-funded-v1.json
```

The public bundle is154 KB and contains no keys, PCZT, witnesses or private pads.
The admission authority is configured separately; never trust a key chosen by a
supplied bundle. Build the [pinned tools](docs/toolchains.md) first. On this machine
the browser can open ignored fixture viewing identities; a fresh checkout uses a
public-only view. [Demo instructions](docs/demo.md) separate recorded proving,
own-result verification, public verification and controlled chain confirmation.
The [seller / bidder entry](docs/local-rehearsal.md) creates a new local auction,
fresh input proofs and signed acceptances, then clears those exact shares with
REP3 and verifies the same certified root before opening your result. Deposits
are synthetic and the localhost helper sees typed demo bids. New-book clearing
does not send payments; the optional native example is a **different recorded
settlement**.

The earlier synthetic run remains available through `./demo.sh --synthetic` and
`./verify.sh demo/artifacts/canonical-nu7-v1`.

This is isolated **regtest on one host**, not public testnet. The book uses fixture
clients/tickets/terms and existing committed input proofs/shares. Admission signatures
attest checked funding; they are not a public ZK funding proof. Public transaction
verification does not prove chain inclusion. No independent operator hosts or
mainnet settlement is claimed. [Exact evidence](evidence/2026-10-05-canonical-funded-nu7-memo-auction.json).

The old deterministic delivery commitment leaked two-candidate refunds; its
privacy claim was withdrawn and an actual attack reproduced the leak. Current
v2 uses a fresh uniform mask plus an independent secret commitment blinder.
See [attack and fix](docs/operator-fill-delivery.md). Plaintext fixture checks
are correctness oracles, not privacy demonstrations.

The one-thread same-PCZT recovery took1847.653s including controlled confirmation
and scans. Six sequential native row services1295.846s dominate; REP3 witness1.423s
and proof10.824s. Public-only replay112.431s. An earlier190.563s attempt stopped on
the unchanged disk guard before signing/reservation and remains separately recorded.
This shared16GiB Mac result is not an isolated speed comparison. The same-PK worker
is already measured separately; no2-minute settlement or parallel speedup is asserted.
See [submission draft](docs/submission-draft.md) for the story, threat model,
verified/unverified claims and demo script.
The [review entry point](docs/judge-review.md) summarizes what to inspect.
[Fresh-source public replay](docs/reproduce.md) separates tool setup, packaging
integrity and actual fixed verification. The review kit contains committed source
and public artifacts; private fixture identities and dependency binaries are excluded.

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

The [local demo](docs/demo.md) presents one canonical six-output memo transaction.
Its fixed verifier rejects corruption before reading the chosen recipient key.
Registered bidder authentication, live browser bid admission and public chain
funding/delivery remain unfinished. The repo stays private until human approval.
