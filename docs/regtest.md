# Actual isolated native chain gate

## Separate NU7 actual-note migration gate

The latest current-SDK native transfer is demonstrated on a separate official
Zebra7.0.0-rc.0 regtest, branch77190ad9 from height2. It uses its own state and
loopback port29342; the working NU6.3 chain below is preserved. Both nodes are
controlled local networks, not public testnet. Do not cross their captures,
wallet birthdays, branch identifiers or transactions.

```sh
python3 scripts/regtest_nu7_node.py prepare
python3 scripts/regtest_nu7_node.py start
python3 scripts/regtest_nu7_node.py mine 2
python3 scripts/regtest_nu7_funding.py --name nu7-bank-new
target/release/darkcall-native-gate --regtest-dkg-init nu7-dkg-new
python3 scripts/regtest_nu7_transfer.py .local/nu7-bank-new .local/nu7-bank-new/private-nu7-notes-104.json .local/nu7-dkg-new --name nu7-bank-to-dkg-new
target/release/examples/regtest_wallet init .local/nu7-recipient-new
python3 scripts/regtest_nu7_transfer.py .local/nu7-dkg-new .local/nu7-dkg-new/private-nu7-notes-106.json .local/nu7-recipient-new --name nu7-dkg-spend-new
```

Build the root initializer/wallet example below and the locked separate SDK using
`tools/nu7-compat/README.md`. Use fresh names and actual reported heights;
104/106 only apply to a fresh chain. Existing wallets/state are never reset.
Before each transfer the driver checks node genesis/branch, canonical funding
block/tx/cmx, maturity/confirmations and current Merkle anchor, then rejects an
altered path before proving. It refuses chain changes during this controlled
construction. Two generated confirmations are followed by exact recipient scans,
source nullifier consumption and source balance reconciliation. This static-tip
driver is unsuitable for an advancing public chain without further work.

The SDK scanner replays actual canonical serialized NU7 transactions, decrypts
native outputs and reconstructs every owned witness against the final node tree.
It does not validate consensus or block PoW independently: RPC and birthday are
trusted. Cached openings are freshly rederived and compared, never blindly
trusted. Six actual capture/cache mutations reject without changing the source.

The two measured transactions are9166bytes each, fee10000zatoshi. Bank→DKG
funds1000000zatoshi in9.410s (native proof5.337s); DKG→recipient pays500000 in
8.666s (proof5.002s). The second reloads existing pinned2-of-3 DKG packages,
validating participant IDs, group/FVK, threshold and secret/public share
correspondence. It reads no escrow master spending key. All three shares still
reside on one host. The first bank transfer uses its ordinary retained key and
local trusted-dealer FROST. Three altered DKG setups reject before proof creation.

Evidence: `../evidence/2026-10-05-nu7-funded-dkg-transfer.json`. This is a native
funding/transfer gate. Four-bid admission and the accepted root are now demonstrated below; the
latest same six-output memo settlement now completes below. Historical funded
auction results below remain NU6.3 and must not be combined into a NU7 E2E claim.

## Fresh NU7 deposits, admission and final root

```sh
python3 scripts/regtest_nu7_deposits.py SOURCE_WALLET CURRENT_PRIVATE_NU7_NOTES COMMITTED_INPUT_BOOK --name nu7-funded-book-new
python3 scripts/check_nu7_claims.py .local/nu7-funded-book-new-escrow .local/nu7-funded-book-new --name nu7-claim-checks-new
python3 scripts/check_nu7_certificate.py .local/nu7-funded-book-new-admission --name nu7-certificate-checks-new
```

This creates a fresh local DKG escrow and six separate recipient wallets. Four
native15100zatoshi deposits use encrypted client-key/nonce memos and fresh signed
instructions tied to actual output cmxs. The Python driver checks the unchanged
canonical source, broadcasts only to port29342, obtains two confirmations and
reconstructs all four exact native notes/witnesses. Admission independently
decrypts the full transaction with the escrow FVK, byte-checks V3 commitments,
verifies client intent against the existing shared implementation, rechecks four
client input proofs and all12 delivered share pairs, then fsyncs12 acceptance
receipts. It does not read maximum bids, escrow custody packages or recipient keys.
After the original real cutoff, all three operators sign exactly the authenticated
accepted root and retain their journal locks. Never alter that cutoff on restart.

The run at110 uses transaction7d7fd7a4…56d1efe8,18634bytes/five actions/25000fee;
native proof9.184s, construction13.140s. Compilation overlapped this measurement,
so it is not an isolated performance comparison. Input proofs/shares, tickets and
terms are fixture actors from an existing committed input book. Thirteen actual
ciphertext/claim/book/receiver/encoding changes and eight certificate/authority
changes reject. The certificate replay reads only public certificate and explicit
caller-trusted authority; no operator keys or journals. The authority is local
trusted setup, not authenticated production service discovery.

Off-chain claim/receipt/final-book/journal source is reused through
`crates/wallet-protocol`; this avoids importing the conflicting old native SDK
into the new SDK. Native decryption and canonical chain provenance remain separate
requirements. A plaintext note supplied to intent validation is not funding
proof. The root still tests its old native ciphertext adapter too.

Evidence: `../evidence/2026-10-05-nu7-funded-admission.json`. The four notes were
Frozen/unspent at admission. The exact certified memo settlement now consumes
all four in the same current-SDK transaction, recorded below. The old funded
auction and synthetic demo remain separate historical evidence.

## Preserved NU6.3 reproduction

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

## Actual NU7 certified six-output preparation

`python3 scripts/regtest_nu7_prepare.py ESCROW CURRENT_NOTES MPC_BOOK
RECEIVER_OPENINGS ADMISSION --name FRESH_NAME` rechecks the owned node snapshot
and accepted notes, then runs actual blinded-v2 REP3 delivery. The new SDK
checks signed frozen book, authenticated receiver openings, canonical witnesses,
exact nullifiers/value conservation and four encrypted receipt memos. All six
V3 output openings match their native commitments. Preparation is9.118s and
eleven private-copy substitutions reject; original Frozen journals stay unchanged.
It reads no recipient keys/custody packages and produces no native bindings or
real threshold signatures. IO may authorize dummy zero-valued native actions.
Existing synthetic finalizer/verifier refuse this distinct actual preparation
scope. Do not interpret this as completed funded auction settlement. See
`evidence/2026-10-05-nu7-funded-six-output-preparation.json`.

## Same prepared NU7 transaction through authorization

The opt-in `scripts/regtest_nu7_settlement.py` takes ESCROW, CURRENT_NOTES,
MPC_BOOK, RECEIVER_OPENINGS and ADMISSION, plus exactly six distinct
`--recipient-wallet WALLET` options and a fresh `--name`. It checks the current
canonical source before proving. All six unchanged native row proofs and the
fixed auction proof must link to the signed final book under the separately
configured authority **before** any real DKG share is loaded or funds reserved.
It then signs exactly four accepted spends, sends only to the owned NU7 node,
records two controlled confirmations, scans the six exact recipients and
finalizes the same transaction in all three durable journals.

`--resume-prepared ORIGINAL_RUN --native-threads 1` preserves the original PCZT,
six row openings, receipt memos and transaction effects. A fresh ignored output
directory is required. The SDK replays canonical bytes, actual input witnesses,
certified recipient commitments and original delivery before proving. Completed
row proofs are verified before reuse. Failed or mismatched proofs are not silently
replaced. Existing journal reservations must match the exact original digest.
The native identity's `scope` string records the unsigned preparation snapshot;
later authorization is established by verifying the actual serialized transaction,
not by changing that identity string.

This controlled-tip driver rejects a chain change during construction. General
post-broadcast/reorg recovery on an advancing public chain is unfinished. Never
start a replacement transaction just because the driver stopped: inspect its
saved signed bytes, node status and all three journals first. Disk/memory guards
remain active; the one-thread option changes scheduling, not circuit statements,
verifiers or trust assumptions. Completion evidence is recorded in `evidence/`.

## Confirmed canonical funded NU7 memo auction

Transaction47019e02e49557ca1e8ef899b94ff1e464a9f29ec14accc298fb0bad3a211938
completes the CURRENT same-run flow:4 accepted actual notes,3 final-book signatures,
blinded-v2 delivery/four encrypted receipt memos,6 fresh native bindings, committed
REP3 proof, independent certified application checks,4 existing-DKG spends, native
proof/signatures, Zebra acceptance,2 controlled confirmations at112,6 exact
recipient scans/canonical paths and3 Finalized journals. Escrow:4 notes spent/zero
balance. Price50/sold3;21790bytes/30000fee. No master SK/full ASK reconstructed.

An initial190.563s run stopped on the unchanged4GiB disk guard before reservation
or signing. The original PCZT/row openings/memos/effects were preserved; recovery
finishes the SAME transaction with one native thread in1847.653s including scans.
This is a paging/recovery experiment on a shared16GiB M1, not a speedup claim.
Full public replay112.431s, four own memo clients and six substitutions22.247s,
eight public substitutions13.929s; default funded demo and full HTTP replay pass.
Public bundle has no chain-inclusion proof; its admission authority is explicit
and separate. Fixture book/keys/operators remain on one host. No public testnet,
malicious MPC or advancing-chain/general post-broadcast recovery is demonstrated.
See [canonical evidence](../evidence/2026-10-05-canonical-funded-nu7-memo-auction.json)
and [demo](demo.md) for public-only and own-client reproduction.
