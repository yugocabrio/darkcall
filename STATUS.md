# DarkCall status

Updated: 2026-10-05 08:23 UTC.

## Current outcome

**The funded local-DKG native auction now passes after exact partial-proof recovery.**
Original four confirmed deposits/receipts → same saved private built PCZT/row salts
→ two existing row proofs reverified, four missing rows generated → private MPC
and certified/native output checks → four 2-of-3 DKG threshold spends → actual
native proof/full-node acceptance → two controlled confirmations → all six exact
wallet receipts and all four consumed escrow notes → three finalized journals.
No escrow SpendingKey/full ASK generated/read/reconstructed by this path. All
three key shares and every REP3 node remain on one Mac, not independent custody.

Transaction 89547f42484258df9958dd2775018a4665c3888363c26a99aacd0e5abb97b859;
4 inputs / 6 outputs, 21,790 bytes, fee 30,000, price 20/sold 3. Resumed construction
886.077s, application 867.097s, native consensus proof 9.247s. This excludes the
initial two native rows; it is not a cold benchmark or a DKG speedup comparison.
Exact private PCZT bytes, canonical rows and all native cmxs equal the interrupted
source. Max sampled root RSS 2,731,920 KiB, min free disk 18,042,916,864 bytes,
min reported memory headroom 33% (10-second sampling, not exact peak).
Evidence: evidence/2026-10-05-dkg-funded-resumed-auction.json.

Completed/consumed: exact built resume session 97091, log .local/dkg-auction4-resumed.log.
Output .local/regtest-dkg-auction4-resumed-20261005; original partial source
.local/regtest-dkg-auction4-recovery2-20261005 preserved. Node is now at height
114 with tip c3237c7c8a97a5bc3311c5cdbb8a926300b579cfadab0ab4cea32aa9593c4d2d.
Do not reset wallets/chain or recreate original funding/receipts/cutoff.

Public-only certified replay completed/consumed (13748) in 114.527s: native
transaction/signatures, six row proofs, normal C++ MPC and all-three certificate
verify without bids, keys, private openings, masks or PCZT. Seven changed
certificate/authority/book/recipient cases reject. Public bundle
.local/regtest-dkg-auction4-public-20261005; log .local/dkg-certified-public-replay.log.

Signed-finalized restart reconciliation completed/consumed (73183), log
.local/dkg-signed-recovery.log. --resume-signed reverified the original checkpoint,
exact signed bytes, public proofs, six wallet receipts and four spent nullifiers.
No transaction re-send/re-sign, no generated blocks, no broadcast/finalize journal
writes. Node height 114 and the same canonical tip are retained. Separate tx
trailing-byte, binding-signature and action-metadata changes also reject. Evidence
above includes this actual finalized-restart gate. General live-chain reorg/expiry
remains unfinished; the copied Prepared-stage integration gate is recorded below.

PC reboot audit and controlled resource stops remain in separate evidence.
The first rebooted run had no native output checkpoint/reservation/signature;
6 GiB guard later stopped three proved rows at a 6.135 GiB transient. The current
8 GiB root RSS/4 GiB disk/10% system-memory guards completed successfully. They
do not prove host crash cause or guarantee stability. A separate intentional
process interruption after two rows exercised the exact built checkpoint.
Changed cached amount/row rejects before reservation; cached scans replay actual
SDK ciphertext/witnesses and compare the entire freshly derived result.

Partial Prepare/Broadcast/Finalize fan-out now preflights all local journals and
continues only the identical effects and purpose. Existing Prepared reservations
may resume through the same built PCZT; complete signed transactions must use
signed reconciliation. No note unlock/reset/replacement exists. Selected native/
core/wallet regression tests 35 pass, strict selected all-target release clippy
passes, and 11 Python checkpoint/reconciliation/resource tests pass. The separate
offline Prepared-stage native integration now also passes (312.374s): copied
prefixes from the actual DKG journals reconstruct two Prepared and one Frozen
operator. All six cached row proofs reverify; same effects/private MPC/certification
and four DKG spends/native proof pass. Existing reservations receive zero duplicate
writes; only the Frozen copy appends one Prepare. Original finalized journals and
source artifacts remain unchanged. Changed cancellation purpose rejects before
native row proving/signing; four prefix tests pass. No RPC/broadcast/mining in this
gate: notes are historical and already spent. This is not another funded auction
or an actual interrupted Prepare fan-out. Evidence:
evidence/2026-10-05-prepared-dkg-recovery.json. General live reorg/expiry/cancellation
remain unfinished.

**The isolated NU7 compatibility gate passes without changing the working
NU6.3 workspace.** New pinned native SDK: synthetic NU7 v6 PCZT → full private-
field checks → native proof → existing 2-of-3 FROST via canonical bytes → exact
recipient decryptions and public native verification. Total 6.884s, native proof
3.112s, 9,166 bytes/two actions; one-share, digest, branch, trailing-byte and
binding-signature changes reject. The live HTTPS endpoint testnet.zec.rocks now
passes testnet genesis/NU7 identity and actual compact-to-Ironwood-frontier
reconstruction at 4466153. The new SDK also verifies the old confirmed four-funded
auction's public native transaction (1.405s), rejecting treating it as NU7.
Evidence: evidence/2026-10-05-nu7-compatibility.json. No public funding/broadcast,
no application binding migration yet. Standalone lockfile and 277-package declared
license inventory preserve all working old SDK pins/chain/wallets. See
tools/nu7-compat/README.md. Next: actual new-SDK output binding, then funded NU7 and live-chain
freshness/reorg support; do not override the old application's branch guard.

The new-to-working V3 note byte interoperability gate also passes seven random
boundary cases (0.037s): cmx/nullifier, psi/rcm_v3 and canonical FVK/note encodings
match across the two exact Orchard pins. No Rust field/group types are shared.
Changed amounts change commitments; this is sampled semantic compatibility, not
an application binding proof. Native NU7 regression still passes (10.385s total,
3.875s proof). Strict standalone all-target release clippy passes. The updated
295-package inventory includes the old note library; root lock remains unchanged.
Evidence: evidence/2026-10-05-nu7-note-interop.json. Actual new-SDK outputs → existing
binding circuit and public-chain funding/freshness remain next work.

**The four-funded native auction passed on the actual isolated chain.** Confirmed
native deposits → authenticated client/proof/share admission → durable receipts
and all-three final root → private refund delivery → six constrained native
bindings → normal C++ MPC verification → certified recipient/effects checks →
fsynced input reservations → four 2-of-3 FROST spends → native consensus proof →
full-node acceptance → two controlled confirmations → six separate exact wallet
receipts and four actual spent-nullifier detections → three finalized journals.

Construction: 1425.263s (23m45s), application proofs 1406.442s, native consensus
proof 8.091s; 21,790 bytes, four inputs / six outputs, fee 30,000; price 20, sold 3.
MPC proof 8.310s; native row key/proofs/verification 1287.464s. Early build/test work
overlapped this cold run; these are completion timings, not isolated benchmarks.
Public-only certified replay passed in 105.691s, without private openings/keys,
and seven changed certificate/authority/book cases plus three changed tx bytes,
signature/action metadata cases were rejected. Chain inclusion is checked through
the trusted full-node driver, separately from public proof replay.

Transaction: 9d16a088186749dba179df1330fa8a7da6a9343473671c08a381fcdb91fc54da.
Evidence: evidence/2026-10-05-regtest-funded-auction.json. This is one controlled
regtest fixture, not public testnet or production custody. See limitations below.

Four actual isolated-regtest deposits now pass authenticated admission. A native
bank transaction delivered four encrypted 15,100-zatoshi deposit outputs;
18,634 bytes, five actions, fee 25,000, native proof 7.851s, construction 9.540s.
The escrow SDK scanner verified all four unspent notes and canonical witnesses.
Each client claim, all four client ZK proofs and 12 delivered pairs were rechecked
before 12 durable operator receipts. Admission occurred before the original
wall-clock cutoff; afterward all three operators certified exactly the persisted
book. Deposit construction does not read maximum bids. All actors are local;
the bank batches deposits and the prototype still uses fixed fixture tickets and
tiny fixture budgets. This is real regtest funding, not independent client funding.

The funded run (8491) and public replay (14874) completed/consumed. Private logs:
.local/regtest-auction4-gate.log / .local/regtest-certified-replay.log. Construction:
.local/regtest-auction4-20261005; public-only bundle:
.local/regtest-auction4-public-20261005. Source deposits:
.local/regtest-deposit4-20261005; receipts/manifest/journals:
.local/regtest-deposit4-20261005-admission. The node is at height 110. No native
prover is running. Do not reset these wallets or the persistent isolated chain.

Actual isolated-regtest chain funding and threshold spending now pass. A pinned
Zebra 6.3.0 node accepted a native v6/Ironwood transaction spending a matured
shielded coinbase with its actual canonical Merkle witness and 2-of-3 rerandomized
FROST. After two controlled blocks, a separate wallet decrypted the exact
1,000,000-zatoshi payment and built witnesses matching the full-node tree.
Transaction: 9,166 bytes; native proof 4.353s; construction 6.075s; fee 10,000.
A different valid transaction spending the same note was rejected by the node
for its already-spent native nullifier. Changed witnesses fail before proving.

This is a **threshold transfer on an isolated regtest**, not the funded auction
or standard public testnet. Setup retains the bank spending key and uses a local
trusted dealer, not DKG. The node supplies trusted chain inclusion and two
controlled confirmations, not an SPV proof or public-chain finality. Funding,
keys, witnesses and transactions remain ignored. Evidence and reproduction:
`evidence/2026-10-05-regtest-funded-threshold-spend.json`, `docs/regtest.md`.


The price-20 four-bid integration passed using **proved private refund delivery**,
without reading maximum bids or oracle fixture refunds in native orchestration.
It generated all six BN254 Poseidon2/native bindings, verified the committed MPC
auction using normal C++ Barretenberg, checked the actual native outputs/effects
before 2-of-3 FROST signing, generated the Ironwood consensus proof, extracted and
verified the transaction, and decrypted all six recipients. Total 1167.489s;
21,790 bytes, 4 inputs / 6 outputs, standard fee 30,000.

**The earlier offline integration has fabricated funding/anchors. Never broadcast
those earlier synthetic transactions.**
This is an offline synthetic gate, not a chain-funded auction. The original signed
run predates receipt authentication; the later certified public replay authenticates
accepted instructions/recipients under an explicit test manifest. All REP3
processes run on one Mac.

Reference commits: 1bd72df (private-delivery native settlement) and 86efe25
(scanner, claims and durable receipts). Use git log -1 / origin/main for the
current head. The new all-three final-book certificate now links exact accepted
entries and all six recipient commitments to the actual MPC public statement.
Certified public-only native/MPC replay passed in 125.697s with seven altered
signature/authority/book/recipient cases rejected. Funding admission in that historical certified replay was
synthetic; the newer actual-regtest admission is recorded above. Its operator
endorsements still are not a public funding proof.

## Prior jobs and historical measurements

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
  No standard-public-testnet funding or broadcast. Isolated regtest now passes.
  Evidence: test-chain-identification.json and regtest-funded-threshold-spend.json.
- Wallet journal/network tests passed (5); latest regression tests passed (23
  total across core/input/native/wallet). Strict release all-target workspace
  clippy passed (64300 consumed). Python public bundle script compiles.

New library gates: 15 wallet tests pass, including genuine V3 compact ciphertext
decryption and witness updates; an actual PCZT-built full funding ciphertext with
encrypted client-intent memo; three matching Ed25519 receipts with a persisted
per-ticket signature lock. These tests use synthetic inputs and do not establish
chain funding or client-proof admission. The two new final-book tests check
complete local funding/receipt matching, omission detection, cutoff/authority
binding, root equivocation after restart, malformed MPC inputs and reorg pauses.
Selected core/input/native/wallet regressions pass (33); strict workspace all-target
clippy and Python syntax checks pass.

Next highest priority: complete DKG public replay/signed restart reconciliation,
then funded NU7/native application compatibility and recovery. The isolated
NU7 compatibility gate now passes. Keep the old-branch
reproduction while implementing funded NU7 and live-chain freshness, then
cancellation/restart recovery and client UI. Public
source research identifies 77190ad9 as NU7, activated at 4465026; a new live probe
at 4466083 confirms the branch mismatch with the old SDK. Do not relax native
branch guards. Selected regressions pass (34), receipt transport tests and strict
workspace all-target clippy pass; the read-only diagnosis compiles and was probed.
Dependency inventory records 430 main-workspace packages and 277 standalone NU7
packages with explicit pins/no missing declared licenses. Strict standalone all-
target release clippy and upgrade/genesis boundary test pass.

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
  once for settlement or cancellation. Fsynced journal preserves this across
  restart and excludes concurrent local writers. The new in-memory native scanner
  decrypts Ironwood notes and builds spend paths; live RPC/persistence/reorg wiring
  remains. Signed receipt issuance locks exact authenticated instructions before
  signing. All-three final certificates now require the complete frozen local
  roster and previously locked receipts, lock the final book before signing,
  and bind the exact MPC root/book/refund and configured seller/operator
  commitments. These are operator attestations of admission, not funding proofs.
  Public replay takes trusted configuration as an explicit external input.

## Trust and unfinished guarantees

REP3 privacy assumes at most one semi-honest corrupt party. Two colluding parties
can reconstruct bids; all three are required for computation. Funds use 2-of-3
custody: two colluding signers can steal; there is no unilateral user refund.
Operators know recipients/refunds/viewing metadata. Output inference remains:
with four eligible bids and three units, the sole loser's maximum equals the
public K+1 price. Zero-price winner flags still need private delivery.

Unfinished: public-testnet native support/broadcast, broader funded cases,
persistent wallet scanning/reorg recovery, general service/client admission and signing, cancellation transactions, deployed independent hosts, UI,
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
