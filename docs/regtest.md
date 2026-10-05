# Actual isolated native chain gate

The pinned Zebra 6.3.0 ARM Linux binary runs in Docker with two CPUs, 2 GiB,
no public peers, persistent ignored state and host RPC only at 127.0.0.1:29332.
The driver checks the regtest genesis and NU6.3 branch 37a5165b before mutations.
It never sends to an external chain or resets an existing wallet/node.

```sh
cargo build --release --locked -p darkcall-native-gate -p darkcall-wallet-bridge --example regtest_wallet -j 2
cargo build --release --locked -p darkcall-native-gate -j 2
python3 scripts/regtest_node.py start
python3 scripts/regtest_node.py mine 2
python3 scripts/regtest_funding.py --name regtest-funding-new
RAYON_NUM_THREADS=2 python3 scripts/regtest_spend.py .local/regtest-funding-new .local/regtest-funding-new/private-notes-104.json --name regtest-transfer-new
```

Use fresh names and the height actually reported by the funding run. Mining two
blocks is required only on the initial chain. Funding mines two shielded
coinbases and 100 maturity blocks, then captures actual full transactions and
trees. The SDK scanner decrypts native V3 notes, tracks nullifiers and checks every
Merkle path against the actual full-node frontier. Subsequent funding runs with
an existing birthday rescan instead of minting additional funds. Existing capture
files are preserved; repeated captures at the same height fail explicitly.

The spend driver checks source block/tx/cmx, maturity, current tip and anchor,
rejects an altered witness, builds PCZT v6, proves native Ironwood, signs with
2-of-3 rerandomized FROST and sends only to this local node. It mines two blocks,
checks canonical inclusion, and scans a separate recipient wallet for exact
payment and witnesses. A different native-valid signed transaction using the
spent source note was rejected by Zebra for an already-spent nullifier.

The measured transfer used one input, two outputs/actions, 9,166 bytes, a
10,000-zatoshi fee and a 1,000,000-zatoshi payment. Native proof took 4.353s;
construction including proof/signing took 6.075s. This excludes mining/scanning.
The funding and resulting notes are real consensus-validated regtest notes;
the node has controlled blocks and disables PoW. No public-chain finality claim.

Keys, associations, openings, private provenance and binaries remain under
ignored .local. The bank retains its full spending key; the signing test uses a
local trusted dealer and local signers, not DKG or independently operated custody.
The scanner trusts the local full-node RPC and checkpoint; it is not an SPV proof.
The gate proves a threshold transfer, not auction admission, MPC settlement
binding, live reorg recovery or crash-safe broadcast reconciliation. The confirmed four-funded auction is recorded below.

```sh
python3 scripts/regtest_node.py status
python3 scripts/regtest_node.py stop
```

Stopping preserves the chain and wallets. Official binary/archive/image hashes
and measured results are recorded in the corresponding evidence JSON. Public
lightwalletd branch checks remain unchanged; the public endpoint returning
77190ad9 is NU7 and is not compatible with the main workspace snapshot. The
separate tools/nu7-compat gate now passes without changing this reproduction.

## Four actual deposits and certified admission

```sh
cargo build --release --locked -p darkcall-native-gate -p darkcall-input-tool -j 2
cargo build --release --locked -p darkcall-wallet-bridge --examples -j 2
python3 scripts/regtest_deposits.py SOURCE_WALLET CURRENT_PRIVATE_NOTES COMMITTED_MPC_BOOK_RUN --name regtest-deposit4-new
python3 scripts/regtest_settlement.py .local/regtest-deposit4-new COMMITTED_MPC_BOOK_RUN --name regtest-auction4-new
```

First refresh the bank's witnesses using its original birthday capture. The
four-deposit driver checks canonical source funding, builds a native transaction
with encrypted client-key/nonce memos, mines two controlled confirmations and
scans four exact escrow notes. Each private claim binds actual cmx, the verified
bid/pair commitments and a client-chosen receiver commitment. Fresh independent
receiver wallet keys stay with those wallet processes; native settlement uses
addresses and outgoing note recovery rather than reading their spending keys.

Admission regenerates a client verification key from the circuit, rechecks four
client ZK proofs and all 12 delivered share pairs, decrypts the actual confirmed
outputs, verifies signed instructions and fsyncs three receipts per ticket.
Operator keys are persisted before any receipt signature. The clock rejects
late admission. After the original declared cutoff, a separate command reloads
and verifies all receipt quorums and certifies the exact frozen root under all
three operator identities. Both receipt and final-book signature locks are
persisted. The manifest is a trusted local test configuration, not authenticated
service discovery or a production key ceremony.

The first deposit gate passed with 18,634 bytes, five native actions, fee 25,000,
and four deposits of 15,100 zatoshi. Native proof took 7.851s; construction 9.540s.
A Python empty-vin assumption initially stopped inspection after confirmation.
Recovery checked the existing chain notes and completed admission before the
original cutoff; it did not remint or change that deadline.

These client actors share one bank and one machine. Tickets, conditions and
100-zatoshi budgets are fixture inputs, not funding-derived IDs or product-sized
auction economics. The subsequent actual funded auction now passes the complete six-output flow;
the admission result is still separate from native settlement and chain checks.
The settlement driver rejects a substituted receiver before proving, binds the
certified recipients to the MPC statement, checks all six native outputs before
FROST, and fsyncs all three funding reservations. After canonical broadcast it
scans each of six recipients, reconciles actual spent escrow nullifiers, and
records broadcast/finalization in the three journals. Interrupted stages need
explicit reconciliation; restart-safe automatic broadcast recovery remains work.

## Confirmed four-funded auction result

The first funded run passed at height 110: four native inputs, six outputs, 21,790
bytes and a 30,000-zatoshi fee. Construction took 1425.263s, including all six
application binding proofs and independent MPC verification. The actual native
consensus proof took 8.091s. Proof/profile timing overlapped early regression and
build work; it is not an isolated benchmark. All six separate wallets decrypted
exact private expected amounts and built witnesses matching the node. The escrow
scanner detected all four actual consumed nullifiers. All three journals moved
through reserved, broadcast and finalized state.

Certified public-only replay took 105.691s and read no private source artifacts.
It checked root/recipient authentication, native transaction proof and signatures,
all six binding proofs and normal C++ MPC verification. Ten modified certificate,
authority, book, tx byte/signature/action cases were rejected. Inclusion/finality
is still trusted full-node evidence, not part of the public application proof.
Recorded evidence: ../evidence/2026-10-05-regtest-funded-auction.json.

Public testnet has moved to NU7 (77190ad9 at 4465026), while this reproducible gate
uses native NU6.3. A public-network flow also needs advancing-chain reconciliation
and an expiry allowance appropriate to measured proving time; requiring an
unchanged tip throughout this controlled gate is not viable on a live chain.


## Local DKG escrow

Add `--dkg` to `scripts/regtest_deposits.py` for the new escrow setup. It runs the
pinned RedPallas FROST DKG part1/part2/part3 for three local participants and a
2-of-3 threshold. Each participant obtains matching public group information;
the wrapper normalizes both shares and group key to even Y. Fresh native `nk`
and `rivk` viewing material combines with that group key into a validated FVK.
No escrow `SpendingKey` or full ASK is generated or reconstructed by this path.
The bank and recipient wallets remain ordinary wallets with their own keys.

```sh
python3 scripts/regtest_deposits.py SOURCE_WALLET CURRENT_PRIVATE_NOTES COMMITTED_MPC_BOOK_RUN --name regtest-dkg-deposit4-new --dkg
python3 scripts/regtest_settlement.py .local/regtest-dkg-deposit4-new COMMITTED_MPC_BOOK_RUN --name regtest-dkg-auction4-new
```

The FVK, three separate operator key-package files and public-package file stay
in a fresh 0700 ignored directory with 0600 files. Each file and containing
directory is synced before the final readiness marker. Existing names are never
overwritten. Scanner/admission read only the FVK. An incomplete setup or a mix of
view-only and master-key formats is rejected. Signing reloads canonical packages
and checks all identifiers, threshold, group key, individual public shares and
their correspondence to the secret shares before use.

The tests verify native compatibility for all three two-party quorums, reject a
single share and changed digest, and reject a substituted operator package.
Another wallet test covers interrupted readiness and master-key aliases.
Authenticated DKG broadcast/private transports and isolated operator hosts are
not implemented: the local initializer and signing process hold all three
shares, so this does not demonstrate independent custody. DKG removes the
retained escrow master key from the prototype's construction; it does not make
a compromised single demo host safe. Recovery/cancellation remain separate work.

## Restart after a PC/process interruption

Restart Docker and `python3 scripts/regtest_node.py start`; existing state is
preserved. Inspect the run log, private checkpoint and all three journals before
starting another proof. A stop before any reservation/complete signed transaction
can reconstruct against the original confirmed funding with a fresh output name.
Do not reset funding, change the cutoff/root, or release reserved notes.

New construction records an immutable ignored recovery input checkpoint and an
unapproved exact private PCZT before row proving. Approved PCZT is separately
fsynced before reservation. Pre-reservation continuation now uses the built checkpoint and verifies existing
row proofs under the trusted circuit against the exact native cmxs/row/context.
Frozen journals can start reservation. Exact same-purpose Prepared reservations
now also pass a complete native/MPC/FROST construction from private copied crash
prefixes. This offline gate uses historical notes that are now spent; production
journals are never truncated or edited. Never unlock or replace reserved inputs.

```sh
python3 scripts/regtest_settlement.py ORIGINAL_DEPOSIT_RUN ORIGINAL_MPC_BOOK_RUN --name FRESH_RESUMED_NAME --resume-built ORIGINAL_PARTIAL_NAME
```

The PCZT's pending real nullifiers must match exactly the four accepted source
notes; native version, pool shape, fee, recovered outputs, row fields and original
input checkpoint are rechecked. Existing row proofs are cryptographically verified,
not trusted because their files exist. Missing rows alone are generated. Changed
or truncated proofs fail closed; they are not silently overwritten.

For a complete signed construction, use the SAME existing settlement name:

```sh
python3 scripts/regtest_settlement.py ORIGINAL_DEPOSIT_RUN ORIGINAL_MPC_BOOK_RUN --name EXISTING_SETTLEMENT_NAME --resume-signed
```

This validates original funding/receivers/authority/certificate hashes and a
canonical original chain checkpoint, audits the exact reservation in every
journal, and publicly reverifies all proofs/signatures and signed bytes. It
checks whether the exact tx is already confirmed/in the mempool. RPC failure
is not interpreted as missing broadcast. Confirmed transactions are not sent
again, and only missing controlled confirmation blocks are generated. Recipients
and escrow re-scan full native ciphertexts/nullifiers; complete cached captures
and openings must equal a freshly computed result. No cached balance alone
authorizes finalization. Reorg/expiry/changed-chain cases fail closed without
unlocking inputs or creating replacement effects. Existing pre-feature runs
without a recovery input checkpoint require an explicit audit/migration.

Actual finalized signed-recovery now passes on the funded local-DKG auction in
`evidence/2026-10-05-dkg-funded-resumed-auction.json`. It reverified native/application
proofs and all actual SDK receipts/nullifiers, with zero re-sends, generated blocks
or journal writes. The original canonical height 114/tip and transaction are retained.
The earlier signed-recovery-gates file records the preceding implementation tests,
not the later actual chain result.
The first monitored rerun reached three verified rows before a conservative
6 GiB RSS guard stopped it (6.135 GiB transient, 70% reported memory headroom).
The next measured run uses one prover with an 8 GiB root RSS ceiling and separate
4 GiB disk/10% macOS memory-headroom guards. Observations are private/ignored.
These are operational limits, not a hardware-crash guarantee or weakened proof.

## Actual funded DKG built-stage recovery result

The source four native deposits and certified book survived the PC reboot. A
subsequent controlled process interruption after two row proofs left an exact
private built PCZT and all three Frozen journals. `--resume-built` reconstructed
the SAME PCZT bytes, canonical private rows and native output commitments,
reverified the two existing proofs and generated only the four missing rows.
All application checks passed, four DKG 2-of-3 signatures and a native consensus
proof succeeded, and Zebra confirmed the resulting tx at height 114. Six wallets
decrypted exact payments/refunds and escrow detected all four spent notes; all
three journals finalized. No escrow SpendingKey/full ASK was generated, read or
reconstructed by this setup. All key shares still reside on one host.

Tx: 89547f42484258df9958dd2775018a4665c3888363c26a99aacd0e5abb97b859.
21,790 bytes; fee 30,000. Resumed construction 886.077s (excludes original two
row proofs), native consensus proof 9.247s. Public-only certified replay passed
in 114.527s and rejected seven modified certificate/authority/book cases.
Evidence: ../evidence/2026-10-05-dkg-funded-resumed-auction.json.

The new exact-reservation helper also accepts a crash between Prepare writes or
during native consensus proving, provided effects AND purpose remain identical.
A different digest, settlement/cancellation substitution, poisoned handle or
later-stage reset is rejected. Outcome fan-out preflights all three journals
before writing. A complete Prepared-stage integration now passes in 312.374s
using copied historical journal prefixes: two Prepared and one Frozen, all six
cached proofs reverified, unchanged effects, four DKG spends/native proof and
one new Prepare only. Cancellation-purpose substitution rejects before row
proving/signing. Original finalized journals are preserved. The offline harness
never calls RPC, broadcasts or mines; these historical notes are already spent.
This is not a second funded auction. Reproduce with:

```sh
python3 scripts/regtest_prepared_gate.py ORIGINAL_DEPOSIT_RUN ORIGINAL_MPC_BOOK_RUN ORIGINAL_BUILT_RUN COMPLETED_PROOF_RUN --name FRESH_OFFLINE_NAME
```

Evidence: ../evidence/2026-10-05-prepared-dkg-recovery.json.
The actual funded built-stage run above started from Frozen. If complete signed bytes exist, use --resume-signed
instead of constructing/signing again. General live-chain reorg/expiry recovery
remains outside these controlled-chain paths.
