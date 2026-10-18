# DarkCall demo guide

```sh
python3 scripts/regtest_nu7_node.py start   # only for Settle on Zcash
./demo.sh                                   # http://127.0.0.1:8877
```

The page offers two experiences:

- **Try DarkCall** walks one auction through Create → Bid → Clear → Result →
  **Settle**. Settle funds the same sealed book on the isolated NU7 regtest and
  pays it out in one shielded transaction. Then you open your own shielded
  receipt. Details and exact scope: [local-rehearsal.md](local-rehearsal.md).
- **Proof it settles** shows an auction that was created, funded, cleared and
  settled through this UI on Oct 19, tx `e3ffa436…84769754`. You can open each bidder's
  shielded receipt in about 3 s or verify the whole settlement in about two
  minutes when there is no time to settle a new auction.
  `./demo.sh --fixture-recorded` shows the Oct 5 settlement with fixture bids,
  `47019e02…a211938`, instead.

`./demo.sh --public-only` serves a key-less view. It verifies settlement and
rejects tampering, and refuses own-receipt opening without a key.

## Capture-ready demo plan (about 2 minutes edited)

Settle takes minutes, so the human records it in one continuous session and
cuts the waits, labelling each cut. Never shorten a deadline or replace a stage
with a recording of another auction.

1. **Before recording.** Start the node and `./demo.sh`, open the page and
   check the clock. Run `python3 scripts/regtest_nu7_node.py status` and keep
   about 15 GB of disk and 10 GB of memory headroom. Close other heavy jobs.
2. **0:00–0:20 Create.** Keep the default terms (3 units, reserve 20, budget
   100) and set the close time to about 2 minutes ahead. Say: the price will be
   public; operators never reconstruct the bids (here all roles share one
   machine, and the local helper sees the typed demo bid).
3. **0:20–0:45 Bid.** Seal four demo bids, for example 77, 61, 43, 21. Each
   shows "Bid accepted" after a client ZK proof, three share checks and three
   operator signatures. Mention that this local helper sees the typed demo bid.
4. **0:45–1:10 Fund, then clear.** After the deadline (cut the wait): Certify
   closed book → Fund on Zcash. Show the deposit txid and "same root"; cut the
   about 2 minute wait. Then Clear this auction (about 9 s of real MPC) → Open my
   result (WON / refund entitlement). Optionally show Tamper with proof.
5. **1:10–1:35 Settle on Zcash.** Press it. Show the four live stages and cut
   the waits with on-screen elapsed times: MPC delivery, Bind every output, Sign
   & confirm, Public check. Show the settlement facts:
   the transaction ID, the same sealed book root, the price matching the browser
   clearing, and 2 confirmations.
6. **1:35–1:55 Your private receipt.** Open my shielded receipt shows WON or LOST
   and the refund received, decrypted with your own key only. Then Tamper with
   receipt shows the refusal before the key is read.
7. **1:55–2:10 Close.** Show the trust model in one card: semi-honest REP3 with
   at most 1 corrupt party, 2-of-3 custody, operators see outcomes, regtest on
   one host. Show the verifier command for the public bundle.

Expected timings on the 16 GiB M1: bid about 1 s each, clearing about 9 s,
funding + admission about 2 min (it includes a 120 s admission window), and
settlement about 15 min. Public check about 2 min,
own receipt about 3 s.

## Recorded settlement story

1. Four committed maximum bids remain secret-shared; recorded price/units are public.
2. Choose a fixture bidder and **Open my result**. Both fixed C++ proofs,
   exact context/price/sold and signed admission must pass before the own FVK is read.
   The client verifies native authorization, decrypts its memo, checks the caller's
   expected ticket/book and matches native amount to refund. No other identity copied.
3. **Run tamper check** rejects an altered proof before key access/decryption.
4. **Verify public settlement** checks the SAME native transaction, six binding proofs,
   fixed MPC/receipt proofs and signed accepted book under the configured authority.

Receipt verification and full six-row settlement verification are separate stages.
Recorded/previous results are labeled. The separate browser entry uses real
input proofs/receipts with explicitly synthetic funding; it is not live chain
admission or production bidder authentication. Selecting a recorded ticket is not login.
The judge-facing view leads with sealed bids, price and the own result. Advanced
receipt checks, transaction/root and trust/limitations are collapsed by default.
A receipt pass displays auction/receipt and native checks; the settlement badge
appears only after full public verification passes. Previous server verification
is labeled separately and does not restore that badge after a page reload.
The server binds IPv4 loopback, serves only three static files and a same-origin/
session-token bounded API. It exposes no keys, pads, raw memos, signing or RPC.
All operators know refunds/status; two MPC colluders can reconstruct maximum bids.

The historical synthetic story remains reproducible separately:

```sh
./demo.sh --synthetic
./verify.sh demo/artifacts/canonical-nu7-v1
```

That transaction52555e25…151ab3b has synthetic notes/dealer signing and no funding
certificate or confirmations. Do not attribute it to the funded DKG run.

## Regenerate the synthetic run

Build the pinned tools. Make a fresh committed four-bid book with
`scripts/run_mpc_gate.py` (bids 90/70/60/50, reserve 20), then:

```sh
python3 scripts/run_nu7_auction.py BOOK_RUN --name FRESH_NAME --receipt-memos
python3 scripts/public_bundle.py .local/FRESH_NAME --export .local/FRESH_PUBLIC
python3 scripts/check_canonical_receipts.py .local/FRESH_NAME .local/FRESH_PUBLIC
./verify.sh .local/FRESH_PUBLIC
./demo.sh --synthetic --run FRESH_NAME --bundle .local/FRESH_PUBLIC
```

Use new simple lowercase names; existing proofs/keys/journals are preserved. The
current full gate needs 10 GiB initial disk, 4 GiB runtime, 8 GiB resident ceiling and
10% macOS memory headroom. Never lower these to finish a demo. Synthetic funding
cannot be broadcast. This regeneration does not reproduce historical funded DKG.

## Actual checks

```sh
python3 scripts/check_demo_http.py --canonical --funded
python3 scripts/check_demo_http.py --canonical --funded --settlement-security-only
python3 scripts/check_demo_http.py --canonical --funded --settlement-only
python3 scripts/check_funded_public.py demo/artifacts/canonical-nu7-funded-v1 demo/authority/canonical-nu7-funded-v1.json --name FRESH_CHECK_NAME
python3 scripts/check_public_boundary.py --funded --authority demo/authority/canonical-nu7-funded-v1.json
```

Four own receipts, 15 API/input refusals and a corrupt-proof refusal pass on the
same six-output run. Public-only checkout checks use `--public-only` and a separate
local test port. Native transaction/proof/action/layout substitutions are covered
by `scripts/replay_nu7_auction.py`; source artifacts remain unchanged.

## Proving latency

The historical synthetic shared-host cold run took1015.173s; independent public replay 99.323s,
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

## Same funded run reproduction and measurements

Follow [actual NU7 deposits/admission and settlement](regtest.md). Use fresh deposits
and names: the completed four source notes are spent. Never reset their journals or
re-sign with replacement bytes. The settlement driver accepts `--resume-prepared`
for the exact unchanged PCZT/rows/memos and `--native-threads 1` scheduling. It is
a controlled-tip experiment; advancing-chain and general post-broadcast recovery
remain unfinished. Existing guards and cryptographic statements are unchanged.

Export a completed run with `scripts/funded_public_bundle.py RUN --export BUNDLE`.
Keep its admission manifest as a separate deliberately configured authority.
`SDK funded-receipt-export RUN INDEX ACTOR_WALLET` sets up only that actor's FVK
record. The own client is:

```sh
python3 scripts/open_funded_ticket.py EXPECTED_BOOK PUBLIC_BUNDLE TRUSTED_AUTHORITY OWN_FVK_RECORD INDEX --name FRESH_CLIENT_NAME
python3 scripts/check_funded_receipts.py COMPLETED_RUN PUBLIC_BUNDLE TRUSTED_AUTHORITY
```

The current recovery took1847.653s including confirmation/scans: cold native key
201.875s, six row services1295.846s, native consensus proof14.088s. Actual REP3
witness1.423s/proof10.824s; public replay112.431s. Shared16GiB M1/one native thread;
not isolated performance or a claimed speedup. The separately stopped190.563s
run is preserved. No larger Linux2/3/6-way results are available. Exact public,
client, HTTP, resource observations and limitations are in
[funded canonical evidence](../evidence/2026-10-05-canonical-funded-nu7-memo-auction.json).
