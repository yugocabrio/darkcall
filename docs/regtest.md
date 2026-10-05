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
binding, live reorg recovery or crash-safe broadcast reconciliation. Four actual
funded claims and their certified auction are the next integration step.

```sh
python3 scripts/regtest_node.py status
python3 scripts/regtest_node.py stop
```

Stopping preserves the chain and wallets. Official binary/archive/image hashes
and measured results are recorded in the corresponding evidence JSON. Public
lightwalletd branch checks remain unchanged; the public endpoint returning
77190ad9 is not compatible with the current SDK snapshot.

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
