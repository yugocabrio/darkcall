# One-run DarkCall demo

```sh
./demo.sh
./verify.sh demo/artifacts/canonical-nu7-v1
```

Open `http://127.0.0.1:8877`. Both commands use the same recorded synthetic
NU7 transaction `52555e25…151ab3b`: four committed bids, blinded-v2 private
refund/winner delivery, four encrypted memos in the same six-output transaction,
six output bindings, MPC proof, PCZT/FROST/native authorization and six decryptions.
Its price is 50 zatoshi and three units sell. This tiny synthetic book is a demo;
no public-testnet funding, admission certificate, DKG or chain inclusion is claimed.
The earlier actual-funded NU6.3 regtest execution remains separate.

After pinned tools are built from [toolchains](toolchains.md), the verifier works
from the committed 140 KB **public-only** bundle. It never needs a bid/witness/key,
PCZT or funding journal. It snapshots bounded allowlisted public files, rewrites
only its own internal MPC path, and invokes the compiled native/Halo2/C++ verifiers.
Manifest checksums detect packaging damage; they are not producer authentication.
Supplied VK bytes must equal a key reconstructed from our pinned circuit.
Wrong/missing artifacts, external MPC paths, symlinks and cross-run contexts refuse.

`./demo.sh` uses ignored synthetic recipient identities on this development machine.
A fresh checkout automatically starts a **public-only** view when those identities
are absent. It can show the recorded sealed book, verify settlement and reject a
changed proof. It explicitly disables own receipt opening without a viewing key.
No recipient key is shipped in Git. `./demo.sh --public-only` selects that view
explicitly. Tool bootstrap/compilation is a prerequisite, not a claimed zero-install
cryptographic verifier.

## Functional story

1. Four committed maximum bids remain sealed; the recorded price/units are public.
2. Select a synthetic ticket and **Verify & open my result**. The fixed C++ flags
   proof and fixed auction proof must pass and share the exact context/price/sold
   before the client reads that one identity. It authenticates the native transaction,
   decrypts the memo and checks ticket/root/refund/status. No other identity is copied.
3. **Run tamper check** submits a real altered proof and refuses before key access.
4. **Verify settlement** checks that same transaction's native authorization, six
   row proofs, fixed MPC proof and receipt statement link using public files only.

The UI separates clearing/own-result verification from settlement verification;
showing the price does not pretend that funds have settled. Recorded and previous
verification results are labeled. There is no fake bid form or live plaintext bid
submission. Client proving/admission, registered identity authentication and chain
delivery are unfinished.

The server binds only to IPv4 loopback, serves three static files, and has a bounded
same-origin/token API with one verification at a time. It serves no repo files,
keys, masks or raw memos, and has no RPC/broadcast/signing operation. Selecting a
ticket is not authentication. All synthetic actors remain on one computer and
operators know refunds/status.

## Regenerate the synthetic run

Build the pinned tools. Make a fresh committed four-bid book with
`scripts/run_mpc_gate.py` (bids 90/70/60/50, reserve 20), then:

```sh
python3 scripts/run_nu7_auction.py BOOK_RUN --name FRESH_NAME --receipt-memos
python3 scripts/public_bundle.py .local/FRESH_NAME --export .local/FRESH_PUBLIC
python3 scripts/check_canonical_receipts.py .local/FRESH_NAME .local/FRESH_PUBLIC
./verify.sh .local/FRESH_PUBLIC
./demo.sh --run FRESH_NAME --bundle .local/FRESH_PUBLIC
```

Use new simple lowercase names; existing proofs/keys/journals are preserved. The
current full gate needs 10 GiB initial disk, 4 GiB runtime, 8 GiB resident ceiling and
10% macOS memory headroom. Never lower these to finish a demo. Synthetic funding
cannot be broadcast. This regeneration does not reproduce historical funded DKG.

## Actual checks

```sh
python3 scripts/check_demo_http.py --canonical
python3 scripts/check_demo_http.py --canonical --settlement-security-only
python3 scripts/check_demo_http.py --canonical --settlement-only
python3 scripts/check_public_boundary.py
```

Four own receipts, 15 API/input refusals and a corrupt-proof refusal pass on the
same six-output run. Public-only checkout checks use `--public-only` and a separate
local test port. Native transaction/proof/action/layout substitutions are covered
by `scripts/replay_nu7_auction.py`; source artifacts remain unchanged.

## Proving latency

The complete shared-host cold run took 1015.173s; independent public replay 99.323s,
and HTTP public verification 101.076s. Key preparation 108.596s, sequential six
proof/verify services 667.790s, native consensus proving 7.805s. Do not sum overlapping
sub-scopes into a second total or compare them to an unrelated fixture as a controlled
speedup. See the benchmark evidence for 10-second resource samples, not true peaks.

`scripts/benchmark_warm_rows.py SOURCE --workers 1 --rows 0 --rounds 2` is a pipe-only
persistent shared18-PK experiment. It prepares one key and makes fresh proofs for
repeated jobs. It does not generate a signing approval. On this Mac concurrency 1
is enforced; 2/3/6 require sufficient measured Linux RAM and all resource guards.
The original full auction command still uses its cold path; the standalone
worker's timing does not establish a 2-minute end-to-end settlement. Its measured
cold key was111.984s and two fresh row0 jobs82.310s/92.506s. A warm-produced proof
also passes the independent fixed public verifier in103.105s. Full auction worker
orchestration now passes two distinct complete synthetic auctions with ONE PK.
Larger-host concurrency remains unmeasured.

The complete synthetic memo path can use one warmed shared18 key:

```sh
python3 scripts/run_warm_auctions.py --workers 1 \
  --job .local/COMMITTED_BOOK fresh-warm-auction-1 \
  --job .local/OTHER_COMMITTED_BOOK fresh-warm-auction-2
```

The service prepares the key once before reading auction requests. Its bounded
local JSON pipe can also be kept ready before a book closes. This wrapper sends
jobs only after the ready/previous completion event, preserves10GiB initial disk
headroom and applies the existing8GiB resident guard to the whole owned group.
Each job independently executes fresh v2 delivery, all six proofs, actual MPC,
fixed C++ verification and native application replay before synthetic signing.
An early `clearing_verified` event reveals only the public price/sold and marks
settlement pending; final price/sold must match it. Key params/VK/PK and row proof
creation/verification have separate timers. No RPC/funding/imported keys are
accepted. The original row-only worker still has no signing capability.

Measured warm full service: initialization122.447s (parameters92.015s, VK13.077s,
PK17.355s), then positive/zero jobs901.704s/823.613s. Early clearing13.032s/8.163s
is fixed-proof checked and explicitly precedes settlement. Twelve fresh row proofs,
fixed C++/SDK application replay, four threshold spends and six decryptions per job
pass. Four own receipts and four substitutions per job also pass. Shared Mac
paging prevents a controlled speedup claim. See
[warm full evidence](../evidence/2026-10-05-warm-full-nu7-auctions.json).
