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
