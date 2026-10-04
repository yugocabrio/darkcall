# DarkCall status

Updated: 2026-10-04 23:54 UTC.

## Current outcome

The price-20 four-bid integration passed using **proved private refund delivery**,
without reading maximum bids or oracle fixture refunds in native orchestration.
It generated all six BN254 Poseidon2/native bindings, verified the committed MPC
auction using normal C++ Barretenberg, checked the actual native outputs/effects
before 2-of-3 FROST signing, generated the Ironwood consensus proof, extracted and
verified the transaction, and decrypted all six recipients. Total 1167.489s;
21,790 bytes, 4 inputs / 6 outputs, standard fee 30,000.

**Funding notes and anchors are fabricated. Never broadcast these transactions.**
This is an offline synthetic gate, not a chain-funded auction. Acceptance and
recipient receipts are not authenticated. All REP3 processes run on one Mac.

Latest pushed milestone before current changes: 0407d28. Current work adds a public-only artifact bundle,
transaction tampering checks, a read-only network probe and durable funding journal.

## Active jobs and next action

- Native private-delivery integration session 99380 completed with exit 0 and was
  consumed. Source book: .local/mpc-auction4-20261004T231421Z. Native artifacts:
  .local/native-private-delivery-20261005. MPC settlement:
  .local/mpc-settlement_poseidon4-20261004T234126Z.
- Session 9810 build completed/consumed. Public-only bundle replay 85223 passed:
  134.910s, copied no private files; altered tx binding signature, trailing byte
  and action metadata all rejected. Artifacts: .local/public-private-delivery-20261005.
- Standard testnet genesis and compact/tree tip agree, but testnet.zec.rocks reports
  branch 77190ad9 at 4465703; pinned SDK expects 37a5165b. Probe refuses this chain
  snapshot. Zaino testnet returned 503; ECC/isolated regtest TLS connects failed.
  No wallet funding or broadcast. Evidence: test-chain-identification.json.
- Wallet journal/network tests passed (5); latest regression tests passed (23
  total across core/input/native/wallet). Strict release all-target workspace
  clippy passed (64300 consumed). Python public bundle script compiles.

Next highest priority: identify a usable native Ironwood test chain, receive actual
funding notes, construct canonical witnesses and connect wallet observations to
persistent reservations/finality. Signed acceptance/recipient records remain.
The 15-minute heartbeat is recovery; continue active work across milestones.
No native prover or network probe is currently running. Continue scanner/receipt
implementation while resolving the provider branch mismatch; do not relax checks.

## Measurements and evidence

| Gate | Measured result | Scope |
| --- | --- | --- |
| Committed auction4 | Domain 8192, proof ~3s, 14656 bytes | 4 client ZK proofs, 12 delivered-share checks; altered root/price rejected |
| One-row SHA REP3 | 574.499s proof | ARM AES backend, normal C++ verification passed |
| Six-row SHA MPC | 1654.925s proof; ~1.12GB peak RSS per party | All six native hashes/action mappings and C++ rejection checks passed |
| One-row Poseidon2 MPC | 2.262s proof | Same-sized canonical row; normal C++ verification passed |
| Six native Poseidon2 rows | 808.534s cold total; 6×11200 bytes | Reused native key, sequential proofs; single-host memory pressure |
| Six-row Poseidon2 MPC | 8.170s proof; ~601MB peak RSS among parties | Exact six native rows; normal C++ accepts/rejects price/plan/row |
| First native/FROST integration | 1131.462s; 21790 bytes | Oracle fixture fills, fabricated funding; 6 recipient decryptions |
| First public-only replay | 107.513s; ~542MB RSS | Serialized tx, native signatures/proof, native row proofs, C++ auction proof |
| Private refund delivery | 4.798s MPC proof | Fresh field masks, C++ verification; no maximum-bid reconstruction |
| Different-book full integration | 1167.489s total; 1147.188s application proofs | Price 20, sold 3; proved private refunds; native proof 17.815s; 6 decryptions |

Actual commands, pins, memory and limitations are in evidence/. Key latest files:
- 2026-10-04-native-private-delivery-integration-gate.json
- 2026-10-04-native-transaction-mpc-frost-integration-gate.json
- 2026-10-04-public-native-mpc-replay-gate.json
- 2026-10-04-private-refund-delivery-gate.json
- 2026-10-04-six-row-auction-native-poseidon2-bridge.json
- 2026-10-04-six-row-auction-native-sha-bridge.json
- 2026-10-04-native-poseidon-domain-probes.json (K15/K16 failed with row shortage;
  retain K17 and all constraints.)

Poseidon2 is the measured implementation path; SHA remains a demonstrated baseline.
No custom hash is introduced. Exact pinned BN254 Poseidon2 is emulated over Pallas
using canonical bounded non-native arithmetic and source-cell copy constraints.
Additional native bindings assume classical security and do not constrain ZIP 2005
rseed derivation or ciphertext. Consensus proof and recipient decryption are
separate checks. Application proofs are not native consensus rules.

## Other verified implementation

- Rust oracle/state machine: 12 tests (7 auction tests, 5 funding transitions),
  including 4096 small books and all 24 four-bid permutations.
- Noir shared protocol: 8 tests; bid_input: 4; auction4: 8; settlement4 and
  settlement_poseidon4: 4 each. Shared 224-byte codec and independent SHA vector.
- Client ZK commitment/share binding and all 12 delivered-pair checks before MPC.
- Actual offline PCZT v6/Ironwood, 2-of-3 rerandomized RedPallas FROST, full native
  proof/signature extraction, 6 recipient decryptions. Ordinary and two-zero-output
  shapes pass. Other native input counts remain untested.
- Amount/recipient/hash/cmx/action index substitution, altered v6 effects digest,
  one-share threshold signing, noncanonical arithmetic aliases and source-cell
  mutation are rejected in the applicable gates.
- Private funding ledger rejects duplicate note/ticket reuse, records wrong/late/
  unaccepted deposits, pauses frozen books after reorg and reserves accepted inputs
  once for settlement or cancellation. New fsynced journal preserves this across
  restart and excludes concurrent local writers; no wallet scanner is wired yet.

## Trust and unfinished guarantees

REP3 privacy assumes at most one semi-honest corrupt party. Two colluding parties
can reconstruct bids; all three are required for computation. Funds use 2-of-3
custody: two colluding signers can steal; there is no unilateral user refund.
Operators know recipients/refunds/viewing metadata. Output inference remains:
with four eligible bids and three units, the sole loser's maximum equals the
public K+1 price. Zero-price winner flags still need private delivery.

Unfinished: actual chain funding, canonical witnesses, broadcast/finality,
persistent wallet scanning/reorg recovery, signed acceptance and recipient records,
authoritative root, cancellation transactions, deployed independent hosts, UI,
independent reproduction, benchmark scale and submission requirements. No public
release, mainnet funds, deployed service or contest submission is authorized.

## Pinned environment and autonomy

Nargo 1.0.0-rc.2; coNoir 0.7.0/co-snarks 467a2abbad940d40ed78f38d424cc21920988976;
ARM AES build uses target-cpu=native and aes_armv8. C++ Barretenberg tag
v5.0.0-nightly.20260522, 87aa5283d5b0cab6632ed49fdd25f6548d2ba91f, exact two
checked-in macOS compatibility patches. SDK/native pins are in Cargo.lock and
prior evidence. New backend proto adapter pins zcash_client_backend 0.24.0-rc.1,
tonic 0.14.2 and tokio 1.53.1.

Keys, PCZTs, bids, masks, witnesses, wallet journals and private logs remain ignored
under .local/.deps. No new native prover should run alongside another on this
16GB Mac. Report RSS and memory footprint separately; cold runs include keygen.

Heartbeat darkcall is ACTIVE every 15 minutes through October 28, 2026 23:59 UTC,
in this same chat. Resume AGENTS.md, this file, TASKS.md, current jobs and the latest
human instructions; implement, test, preserve evidence, commit/push useful milestones.
