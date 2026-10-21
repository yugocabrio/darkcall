# DarkCall demo guide

```sh
./demo.sh --chain testnet                   # Fund and Settle on public Zcash testnet
# or, on the owned local regtest:
python3 scripts/regtest_nu7_node.py start
./demo.sh                                   # http://127.0.0.1:8877
```

Testnet mode needs the funded local test bank `.local/testnet-bank-*` (created
with `darkcall-nu7-compat testnet-init`) and reaches `testnet.zec.rocks`.

The page offers two experiences:

- **Run an auction** walks one auction through Create → Bid → **Fund** → Clear →
  Result → **Settle**. Fund makes real shielded deposits for the sealed tickets
  on public Zcash testnet (or the isolated NU7 regtest), and the operators admit them over the same root
  before any clearing. Settle pays everyone out in one shielded transaction.
  Then you open your own shielded receipt. Details and exact scope:
  [local-rehearsal.md](local-rehearsal.md).
- **See a settled auction** shows the auction in the video: created, funded, cleared and
  settled through this UI on public Zcash testnet on Oct 19, tx `be1157c8…a1c8ad67`.
  `./demo.sh --regtest-recorded` shows the regtest six-step run `e3ffa436…84769754`. You can open each bidder's
  shielded receipt in about 3 s or verify the whole settlement in about two
  minutes when there is no time to settle a new auction.
  `./demo.sh --fixture-recorded` shows the Oct 5 settlement with fixture bids,
  `47019e02…a211938`, instead.

`./demo.sh --public-only` serves a key-less view. It verifies settlement and
rejects tampering, and refuses own-receipt opening without a key.

## Reproducible edited video (one real auction)

`tools/demo-capture/capture.mjs` drives one real local auction through the
browser UI in headless Brave at 1920×1080. It runs Create, four bids, Certify,
Fund, Clear, own result, Settle, both shielded receipts and the receipt tamper
check. It waits only on the UI's own verified status text and writes PNG stills
plus a manifest with UTC times, elapsed seconds, SHA-256 and the public
auction/transaction IDs. `scripts/render_demo_video_v2.py` renders the captioned
video from those stills. It checks every frame against the manifest, and every
time cut states the real elapsed time between the two captured states.

```sh
(cd tools/demo-capture && npm ci --ignore-scripts)
node tools/demo-capture/capture.mjs .local/demo-capture-NEW     # ≈ 25 min, needs ./demo.sh + node
python3 -B scripts/render_demo_video_v2.py --output .local/darkcall-demo-NEW.mp4
```

The committed stills and manifest for the rendered draft are in `demo/media/v2/`.
The storyboard is `demo/video-storyboard-v2.json`.

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
   machine, and each bidder's own client, a local service here, sees the typed demo bid).
3. **0:20–0:45 Bid.** Seal four demo bids, for example 77, 61, 43, 21. Each
   shows "Bid accepted" after a client ZK proof, three share checks and three
   operator signatures. Mention that only the bidder's own client sees the typed bid.
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
   at most 1 corrupt party, 2-of-3 custody, operators see outcomes, every party
   on one host, testnet coins only. Show the verifier command for the public bundle.

Expected timings on the 16 GiB M1: bid about 1 s each, clearing about 9 s,
funding + admission about 2 min on regtest (120 s admission window) or about
10 min on testnet (10-minute window plus real confirmations), and settlement
about 15–20 min. Public check about 2 min,
own receipt about 3 s.

## Recorded settlement view ("See a settled auction")

1. The page opens as the public: the price and units sold, and what nobody
   can see (who won, any bid, any refund). The four bids stay
   secret-shared.
2. Choose a bidder and **Switch to Bidder 0N's view**. The fixed C++ proofs, the exact
   context/price/sold and the signed admission must all pass before that
   bidder's own viewing key is read. The client then verifies native
   authorization, decrypts its memo, checks the expected ticket and book, and
   matches the native amount to the refund. The page then sets what that
   bidder sees beside what everyone else sees. No other identity is copied.
3. **Re-run verification** checks the same native transaction, the six
   binding proofs, the fixed MPC and receipt proofs, and the signed book under
   the configured authority (about 2 minutes), and states the result as four
   claims: the price is right, every payout matches the result, the payment is
   real Zcash, these are the accepted bids.
4. **Try to cheat**: raise the price, swap two bidders' refunds, swap in a
   different bid, or fake a bidder's receipt. The first three change a copy of
   the recorded bundle and run the same verifiers on it
   (`scripts/cheat_lab.py`); the last corrupts a receipt proof, which is
   refused before any key is read. Each result names the claim that broke.

Bidders here are listed in canonical ticket order, not sealing order. Choosing
one is a demo convenience, not a login. The settlement badge appears only after
a full public verification in this page session. The server binds IPv4
loopback and serves three static files plus a same-origin, session-token API.
It exposes no keys, pads, raw memos, signing or RPC.

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
