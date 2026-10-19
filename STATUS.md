# DarkCall status

Updated: 2026-10-19 (git HEAD is authoritative).

## Current state, in one screen

- **Public Zcash testnet (Oct 19):** a fresh API-created auction was funded,
  cleared and settled on standard testnet (NU7) through testnet.zec.rocks.
  Deposits `d573a651…3c7b1737` (block 4,518,128), settlement
  `8e6da99b…034f0db7` (block 4,518,182), 2 confirmations each; price 21 / 3
  sold; public check 98 s; receipts 79/79/79/100; all refusals held. Record:
  `evidence/2026-10-19-public-testnet-settlement.json`. Run with
  `./demo.sh --chain testnet`.
- **Product flow (browser and API):** Create → Bid → Fund on Zcash → Clear →
  Result → Settle on Zcash → own shielded receipt → tamper refusal. The same
  sealed tickets, commitments, REP3 shares and accepted root carry through every
  step, and funding precedes clearing.
- **Headline run (six steps, one browser page, Oct 19):** deposits
  `5e9432e5…d72f14be`, root `2d619728…558d12f9`, price 21 / 3 sold. Settlement
  `e3ffa436…84769754`: driver 912 s, public check 105 s. Ticket 01 WON / 79 and
  Ticket 04 LOST / 100 from their own memos; tamper rejected before key use.
  The bundle is `demo/artifacts/browser-auction-nu7-v2`, served by `./demo.sh`
  as "See a settled auction".
- **Video:** `demo/media/v2/darkcall-demo-v2.mp4` (119 s), edited stills from
  one real auction captured by `tools/demo-capture/capture.mjs` (auction
  `b9c23d7d…`, settlement `e3056995…51caf779`). Frames are SHA-256-checked
  against the manifest, and time cuts are computed from capture timestamps.
- **Scripted E2E:** `scripts/check_fresh_settlement.py`, last run tx
  `94a093e9…`. Clear and settle before funding are refused (400), a tampered
  receipt is refused before the key is read, a foreign capability gets 403, and
  a concurrent proof gets 409.
- **Verification without secrets:** `./verify.sh --funded …` on the host, or
  `scripts/docker_verify.sh` in a clean Linux container built from `git archive`.
  On Oct 19 the script verified the four bundles committed then (about 100 s
  each, strict tamper refusal); video run v3 (103 s) and the public-testnet
  bundle (99 s, strict tamper refusal) are host-verified. Recorded-mode HTTP check: 4 receipts plus 15 refusals.
  Public-only mode: own receipt refused without a key, tamper refused.
- **Committed bundles (6):** `testnet-auction-nu7-v1` (public testnet), `browser-auction-nu7-v2` (headline),
  `browser-auction-nu7-v3` (video run), `browser-auction-nu7-v1`,
  `fresh-book-nu7-v1` (Oct 18 browser books), and `canonical-nu7-funded-v1`
  (Oct 5, fixture bids).
- **Unchanged limits:** one host runs every party; the localhost helper sees
  typed demo bids; REP3 assumes at most one semi-honest corrupt operator; two
  colluding FROST signers can move funds; operators learn outcomes; the public
  price is the highest losing bid or the reserve; deposits come from one local
  test wallet after the close; isolated regtest, no public chain inclusion proof.

Details of every run follow, newest first.

## Current outcome (Oct 18)

**An auction created in the browser now settles on Zcash.** The fresh book's
own tickets, commitments, REP3 shares and accepted root flow through the
existing funded pipeline. There is no new cryptography and no weakened check:

- Create + 4 sealed demo bids (77/61/43/21) → browser REP3 clearing price 21,
  3 sold, root `1997750157502ab27c2e24a623d6f5879159abb87db7b11a116b8938b3e76f7a`.
- `regtest_nu7_deposits.py` on the browser's `clearing-book/`: 4 actual NU7
  deposits from the local test bank, tx `5d80367f782efa958b60c56a8c277f36b9d42755ca9bd3af20647bf636513673`.
  Zebra accepted it with 2 confirmations, and the escrow scan decrypted 4 notes.
  Admission re-verified 4 client ZK proofs, 12 share pairs and 4 claim
  signatures and issued 12 receipts. After the 120 s window, 3 final-book
  signatures. The funded certificate root and tickets equal the browser
  certificate (121 s).
- Funded settlement of the same book: fresh REP3 delivery, six shared18
  bindings, settlement MPC and 2-of-3 DKG FROST. Zebra accepted
  `54941ffd845509ee5dc7b23e08b0756526bf97b3123017d581c9c40345c76ee6` with 2
  confirmations. Six wallet scans: refunds 79/79/100/79, seller 63, operator
  30000. The escrow spent all 4 deposits and the 3 journals finalized.
- First attempt stopped at the pre-FROST settlement-MPC check: the rebuilt
  workspace lacked `target/release/poseidon_batch` (output-binding bins were not
  rebuilt after the Oct 7 `target/` removal). No reservation/signature/broadcast
  had happened. After building it, `--resume-prepared` reused the exact prepared
  PCZT and re-verified (did not regenerate) all six cached binding proofs in
  95.2 s, completing in 327.9 s (2 native threads). That is a recovery time,
  not an uninterrupted settlement time (see the HTTP run below: 951.1 s).
- Public bundle export + public-only verification passed in 103.1 s (native
  authorization, 6 row proofs, C++ auction proof, accepted book/recipients).
  Four own-FVK receipt openings, about 2.7–2.9 s each, matched the browser
  results: three won with refund 79, one lost with refund 100.

**The same flow passes through the server's own Settle job without any manual step.**
`scripts/check_fresh_settlement.py` ran it over HTTP: create, 4 bids, close,
clear (8.1 s), 4 own results, then Settle. Deposits `f73ef593…bf6dcf`
(funding done at 121 s), binding key from 130 s, rows from 241 s, MPC check at
834 s, settlement `0e77bc23fc1efce0ab63a8710c2f41c3cc85cbb8dcec4bafb6484d37ed2f3325`
done at 1072 s (driver 951.1 s, uninterrupted), public verification 119.0 s,
complete at 1198 s. The root and tickets equal the browser book; price 21 and
3 sold equal the browser clearing. Four own native receipts (3 won/79, 1
lost/100) equal the four browser results. Refusals: settle before clear (400),
a second settle and a recorded proof while settling (409), a corrupted receipt
proof before key read, a foreign capability (403), and non-default terms marked
not settleable. Public state carried no capabilities or private paths. Total
1263.8 s. Evidence: `evidence/2026-10-18-fresh-book-native-settlement.json`.

**Clicked through the actual browser UI as well.** In the built-in browser pane:
create (defaults), four bids, certify, clear (8.6 s), own result (ticket 04 LOST,
refund 100), then Settle. Deposits `d11efd9a…5fd04abd`, settlement
`3fee2c7ce3b64689c28700904a5b31e13272b41030ae5eedd58a6a87b5c0b5d7`
(driver 886.3 s), public check 102.0 s, Settle → verified 1117.0 s. Root
`04728ac2…ccbc739a` matched the browser book. The UI showed all five stages
done, the settlement facts and the who-learned-what panel. The pane was closed
during settlement, so the page-memory capability was lost by design. The four
own receipts were opened through the same server function from the CLI
(79/79/100/79). Its public bundle is committed as
`demo/artifacts/browser-auction-nu7-v1`. `./demo.sh` now serves it as "Proof
it settles", and `--fixture-recorded` keeps the Oct 5 run. Its recorded view
was checked over HTTP: four receipts plus the tamper refusal.

**Whole flow in one open browser page (Oct 19, 00:20–00:45 CEST).** First,
Docker Desktop was restarted with the author's approval. Its 16 GiB VM
allocation had kept macOS swap at 18.8 GiB. Afterwards swap was 7.6 GiB and
free disk 20 GiB. The other project's node (`shielded-swap-zebra-nu7`) was
restarted with `docker start` and resumed at its tip of 6803. Then, in one
page: four bids via the one-click chips, certify, clear (8.4 s), and Ticket 01
WON/79. An earlier Settle press at 9.9 GiB free was refused before any deposit.
Settle gave deposits `b0fe3394…9a5b8ccf` and settlement
`ac06fc31b6777e53cfb1e74cacd57b1abfbad896893fc22242e9fa39c50dc074` (driver
933.0 s), public check 99.4 s, Settle → verified 1161.5 s. In the same page,
**Open my shielded receipt** showed Ticket 01 WON/79 and Ticket 04 LOST/100,
each decrypted with its own key only. **Tamper with receipt** was rejected
before the key was read. Screenshots: `docs/img/05-settled.jpg` and
`06-shielded-receipt-tamper.jpg`.

**Fund before Clear (protocol order) is now the UI and API flow.**
`settle_fresh_book.py` has two phases. `--phase fund` makes the deposits, the
chain admission and the final-book certificate over the same root and writes
`NAME-funded.json`. `--phase settle` rechecks root and tickets, rescans the
escrow at the current tip, then settles. `--phase all` is the old CLI path. The
server adds `/api/rehearsal/fund` as a lock-owning job. Clear is refused until
funding is done whenever regtest settlement is available, and Settle requires
funding plus clearing. The UI has six steps (Create, Bid, Fund, Clear, Result,
Settle) and a Fund section showing the deposit txid and the same-root
admission. Harness run: clear and settle before funding were refused with 400.
Funding took 120.7 s (deposits `e1e4d627…bdded9`). Clearing ran after funding.
Settlement `94a093e9a5ddcb92117ac32beecb5473b1d13b12f556295dfa1594db4663b0d8`
took 885.3 s for the driver and 104.9 s for the public check. The four own
receipts equal the browser results, and all refusals pass; total 1189.2 s.

New code: `scripts/settle_fresh_book.py` is the orchestrator: tool preflight,
bank rescan, deposits/admission, settlement, export/verify, own-FVK export and
public progress events. `scripts/rehearsal.py` adds a Settle job that owns the
proof lock, a public progress view, and own native receipt open/tamper.
`/api/rehearsal/settle` is new. The UI adds a fifth step **Settle** with live
stages, settlement facts, the own shielded receipt and a receipt tamper check.
`scripts/check_fresh_settlement.py` is the HTTP end-to-end harness. The native
path stays pinned to the default terms (3 units, budget 100, reserve 0/20); other
terms show "not settleable" in the UI.

**Clean-room Linux reproduction of the public verifier.**
`docker/verify/Dockerfile` builds Noir rc.2 (published digest), the co-snarks
CRS, patched Barretenberg `bb` (566.5 s), native-gate and the SDK (449.7 s)
inside `rust:1.93.0-trixie` from a `git archive` only. It then verified
`browser-auction-nu7-v1`, `fresh-book-nu7-v1` and `canonical-nu7-funded-v1` in
323 s and cryptographically refused a corrupted receipt proof for each
(evidence `2026-10-18-clean-room-linux-verification.json`). It exposed two
defects, both fixed:

- `.gitignore`'s `**/*witness*` hid `vendor/halo2_gadgets/src/ecc/chip/witness_point.rs`,
  so a fresh clone could not build.
- The exact patch check depended on Git's clone-size hash abbreviation; it now
  uses `core.abbrev=7`.

A later full rebuild during 19 GiB host swap drove free disk below 1 GiB. That
build stopped. I removed my verifier image and build cache, dropped the Docker
VM page cache, and deleted only rebuildable caches (root
`target/release/{deps,build}` and `.deps/frost-tools/target/debug`). Free disk
recovered to about 10 GiB. Docker Desktop was not restarted, because another
project's node runs there.

Environment notes: the NU7 container had been removed, but its state survived.
`regtest_nu7_node.py start` recreated it at height 112 with tx `47019e02…`
still canonical (2 confirmations). The bank was rescanned at 112 (415571266
zatoshi). Root `target/` was rebuilt with locked release builds of native-gate,
input-tool, wallet-bridge (bins+examples) and output-binding bins. The SDK
binary in `.deps/nu7-compat-target` is unchanged.

Boundaries remain: one host, semi-honest REP3, operators see outcomes, the
localhost helper sees typed demo bids, deposits come from one local test bank
at settlement time (a second, chain-funded admission over the same root), and
this is isolated regtest, not public testnet. No public chain inclusion proof.

## Previous outcome (Oct 5, ead74fe)

**Try DarkCall / Proof it settles** are now visibly separate modes. Landing cards
become a compact switch with each scope still visible. Create/Bid/Clear/Result
progress follows existing proof gates; fresh price and own result sit side by
side. Bids, signed acceptances, technical details and trust assumptions remain
expandable. Visible copy is shorter; helper input visibility is stated before
sealing and fresh results explicitly say no payment was sent.

Actual browser run bc615713…56f3bd:4 fresh accepted bids, real cutoff, signed
root086dc2a7…512cff, price43/sold2, clearing8.284s/proving2.858s, own WON/entitlement57.
Corrupt proof rejects before pads. Switching modes retains the same ticket;
a separate page has no opening capability and keeps Result incomplete.
Recorded47019e02…a211938 winner50/loser100, tamper-before-key and full public
settlement pass; replay110.622s with no private openings or chain-finality claim.
390px landing/create/fresh/recorded views have no horizontal overflow; override
reset. Browser screenshots contain only synthetic/regtest demonstration data.

Video refreshed to110s/11 scenes with seven new verified UI screenshots and an
explicit DIFFERENT recorded-auction switch. Edited stills, no audio/live-proving
claim. Evidence: `evidence/2026-10-05-two-mode-demo-polish.json`.
All fetch calls, verification guards and recorded verification handlers are
unchanged; no backend/API/circuit/bundle/authority edits. JS syntax, HTML ID/ref
checks and diff checks pass. Existing localhost server remains PID63252/port8877.

Next: human visual/video review and actual second-machine reproduction when
another host is available. Fresh wallet settlement and independent operators
remain outside the current demo; avoid new crypto or packaging detours.

## Previous same-book clearing milestone (5ae9bad)

**The new seller/bidder book now clears itself.** Create → four actual input
proofs/12 share checks →12 signed acceptances → real cutoff/3 final-book signatures
→ existing three-party `delivery_flags4/v2` MPC → fixed C++ verification → own
refund entitlement/winner status. This is the same new root/tickets/conditions,
not a preset claim that an old proof applies. Canonical ticket sorting only
renames shared input slots; each unchanged pair is rechecked against its original
client statement and certificate commitments. Neither connection nor clearing
reads plaintext bid-opening files. Exact first169 fields of the fresh statement
are compared against the independently verified signed book before pad export.

HTTP positive case (2 units/reserve10): price43/sold2, fresh clearing9.357s,
REP3 proving3.471s. Zero-price case (3 units): price0/sold3, all refunds100 but
three winners/one loser, clearing8.481s/proving3.468s. Each has4 fresh input proofs,
12 pairs/12 acceptance/3 final-book signatures and four own-result checks against
an independent plaintext fixture oracle (oracle is NOT the MPC demonstration).
Wrong/foreign ticket capabilities refuse before worker start; public GET/copy
contains none. Conditions/ticket/leaf/root substitutions and actual certificate
signature corruption reject; bad proof/certificate stop BEFORE pads are exported.
Completed clear is idempotent, incomplete clear is retained and stopped.

Actual browser create/four seal/certify/clear/open/tamper passes for another fresh
root: price43,2/2 sold,WON/refund entitlement57,8.5s displayed. Mobile390px has
no horizontal overflow; viewport reset. New result is clearly marked synthetic,
with no native payment sent. Helper sees typed demo maxima; local opening token
is page/server memory only, lost on reload, not production authentication or
native memo transport. All parties remain on one host. No native proof, payment,
funding RPC or canonical source-note reuse is performed for these new books.

**New110-second/10-scene video draft** shows the seller form, actual accepted
fresh book, cutoff jump and fresh clearing/result, then explicitly switches to a
DIFFERENT recorded funded native transaction. The fresh screenshots share root
05d33db9…174d25; native scenes still refer only to47019e02…a211938. Edited stills,
not a continuous live auction/proving recording. Rendered1311895bytes; fresh-result
and switch frames visually checked. Ignored MP4:
`.local/darkcall-same-book-clearing-draft-20261005.mp4`; prior drafts preserved.
Evidence: `evidence/2026-10-05-fresh-browser-clearing.json`.

Locked release build,23 existing Rust tests, strict all-target clippy, format,
JS/Python syntax and diff check pass. Recorded verify/settlement methods remain
AST-identical; circuits, separate authority and canonical bundle are unchanged.
Two check-harness issues (hex signature representation and expected error422)
were corrected; positive/zero suites fully pass. A default debug test build was
interrupted after avoidable disk growth; release checks pass without deleting
private checkpoints. Current server: PID63252/session73589, port8877, log
`.local/fresh-clearing-demo-server-release-20261005.log`. No heavy proof job remains.

Next useful work: review this same-book UX/video, visual polish and independent
reproduction when another host is available. Fresh wallet/native settlement
hookup is still separate work; do not claim that the new browser book is funded.

## Earlier entry milestone (271a323; superseded clearing limitation below)

**Seller → bidder entry now works before the recorded result.** New browser
forms create bounded units/reserve/budget/cutoff, fresh conditions and three
admission authorities; the local share link joins that same new auction.
Seal bid creates a fresh existing `bid_input` ZK proof with pinned plaindriver
`--zk`, fixed C++ verification and three checked REP3 bid/salt pairs. The existing
signed claim/receipt primitives issue three verified durable acceptances.
After four complete tickets and the actual cutoff, the existing final-book
certificate verifies three endorsements. Public receipt copying works.
This is explicitly **offline synthetic funding** under a disjoint genesis:
unmined V3 notes and fabricated Observe/Confirm events, no RPC or native signing.
The localhost client helper sees the typed demo maximum; do not call this
operator-blind browser admission. No new MPC auction clearing or native settlement
runs. A prominent transition opens a **different recorded funded book**, without
using the new bid/receipt to choose a fixture winner. Existing recorded native/
public verification methods are AST-identical and cryptographic circuits,
authority and bundle unchanged. `--public-only` disables rehearsal.

Four fresh input proofs/twelve pair checks/twelve acceptance signatures plus
three final-book signatures pass. Per-bid0.547–0.628s in the HTTP gate; actual
75-second clock cutoff,17 rejections including corrupt proof/pair/context BEFORE
journal mutation. Browser create/seal also passes; four additional browser bids
show4/4 and public receipt copy with three signatures and no opening. Shared link
joins;390px mobile has no horizontal overflow. Recorded bidder01 opens WON/
refund50; tamper rejects before FVK access. Public settlement API's five existing
path/key/authority/Origin/token gates pass. A new full six-row verification run
was not needed: both recorded verifier methods and artifacts are unchanged.
Locked release build, strict all-target clippy, Rust format and JS/Python syntax
pass. First compile failed on a wrong public module/API import and stale build
feature selection, then was fixed; first clippy rejected two range-loop lints,
fixed without changing receipts. Initial blob-download event was unconfirmed in
IAB; it was replaced by browser-confirmed public receipt copying. Evidence and
bounds: `evidence/2026-10-05-seller-bidder-rehearsal.json`,
`docs/local-rehearsal.md`. Keys/openings/shares/journals are ignored and preserved.

**Product-loop video draft now includes the new entry.** Three actual browser
screenshots plus an explicit different-recorded-book card lead into the existing
verified winner/tamper/public screens and funding/trust cards. Nine scenes,
110s/1920×1080/1,339,783bytes, no audio, still-image edit. Entry footers say
OFFLINE/SYNTHETIC; only recorded scenes carry the canonical transaction ID.
Entry/transition/trust frames visually inspected; no proving, wallet access or
upload by the renderer. New MP4 remains ignored at
`.local/darkcall-product-loop-draft-20261005.mp4`; old draft preserved separately.
The existing Rust regression tests pass: input-tool4, wallet-bridge library17,
binary2. Public evidence includes scope, exact pins/commands and asset digests.

**Earlier presentation/review milestones (recorded result UI).**
**Judge-facing information hierarchy now leads with the result.** The latest
human request changes the frontend only: “Sealed bids. Verifiable settlement.”,
price/sold, bidder choice, prominent WON/LOST/refund, public settlement and actual
tamper rejection. Receipt steps, transaction/root and trust/limitations are in
three closed disclosures. No backend, circuit, authority, API request or response
acceptance guard changed. Browser winner/refund50, loser/refund100 and corrupted
proof rejection pass against the same funded transaction47019e02…a211938. Full
public verification passes on the new UI; its settlement badge appears only after
that response passes, not on receipt verification or previous server state.
Technical/trust disclosures open correctly. Mobile390px has no horizontal
overflow; temporary viewport reset. JavaScript syntax, diff check and the three
review-kit tests pass. See `evidence/2026-10-05-judge-demo-presentation.json`.

**A captioned110s demo draft is rendered and visually checked.** Six native
browser screenshots show actual verified fixture UI, then two explanatory cards;
1920×1080, no audio,1,381,053bytes. It is explicitly edited stills, not a live
auction/screen recording or110-second proving claim. Screenshot/storyboard and
renderer are tracked; MP4 remains ignored at
`.local/darkcall-judge-demo-draft-20261005.mp4`. Winner/public/trust frames were
inspected. Same canonical transaction throughout; regtest, privacy/custody and
operator outcome visibility stay explicit. No upload/submission performed.

**Converging on review/reproduction without changing cryptography.** A local
committed-source packager creates deterministic archives of source/public
artifacts with separate authority, exact revision/tree/lock/file digests and a
short judge entry point. It excludes Git metadata/ignored runtime/dependencies;
unsafe paths, links/submodules, private runtime names, high-confidence credential
markers, oversized members and overwrite attempts refuse. Three focused tests
exercise ignored/uncommitted sentinels, deterministic output, malformed archives,
expected revision and unsafe tracked paths. All399 candidate source files pass
the bounded path/marker check. This is not exhaustive secret detection, producer
authentication, proof verification or independent-machine reproduction.
Read-only public replay preflight passes on this Mac: exact C++ pin/two patches,
three committed CRS blobs, Nargo rc.2, two compiled circuits/native helpers and
both unchanged locks. It reads no private runtime or node and starts no proof.
`docs/reproduce.md` gives the missing fresh-source steps and notes that Rust
helpers embed their build directory: rebuild at the final location. Current SDK
README now distinguishes the completed funded run from historical components.
No new proof, signature, circuit or public trust root is introduced. Actual
revision61337e8 kit export/audit and fresh public-only extraction pass:411 source
files,1,309,909bytes, no dependencies/private identities/Git metadata copied.
Missing identity refuses409; absent compiled tools report incomplete prerequisites.
This is same-host packaging smoke, not cryptographic/second-machine reproduction.
This kit includes the simplified UI and screenshot/storyboard/renderer sources.
Its exact archive digest and0.253s smoke scope are recorded in
`evidence/2026-10-05-committed-review-kit.json`; the evidence commit is later than
the packaged source revision to avoid a self-referential archive digest.
Second-machine/Linux access is still unprovided. Optional hardware question is
pending. Automation
remains removed; no public release/deployment/submission is performed.

**One actual funded NU7 transaction now completes the certified memo auction.**
The exact prepared PCZT resumes unchanged as transaction47019e02e49557ca1e8ef899b94ff1e464a9f29ec14accc298fb0bad3a211938.
Actual four accepted15100-zatoshi deposits → signed book/recipient authentication →
blinded-v2 delivery/four encrypted refund memos → six fresh shared18 binding proofs →
fixed committed REP3 proof → independent SDK application checks → four existing
2-of-3 DKG spends → native authorization → Zebra acceptance → two controlled
confirmations at112 → six exact canonical recipient scans → three journals Finalized.
Escrow scanner confirms exactly four source notes spent and zero balance. No master
SK/full ASK reconstructed; no recipient key read during construction. Price50/sold3,
21790bytes/30000fee. Recovery1847.653s, native proof14.088s; one native Rayon thread.
The stopped190.563s attempt remains separate and preserved. Same circuits/verifiers;
no speedup claim. Public-only replay112.431s and full HTTP replay110.112s pass on the SAME transaction.
Four own clients+six substitutions22.247s; eight public substitutions13.929s.
Browser four openings/15 boundaries17.990s; public-only no-key view1.169s.
Fixed proofs/accepted-book authentication happen before own FVK read. Default
`./demo.sh` and `./verify.sh --funded ... --authority ...` now use this run.
The153557-byte public bundle has19 artifacts+manifest, no private material; its
trusted admission authority is a separate public setup file. Evidence:
`evidence/2026-10-05-canonical-funded-nu7-memo-auction.json`.
All operators/fixture actors remain on one host on isolated regtest, not public
chain. Signed admission attests funding; it is not a public ZK funding proof.

**Current local jobs.** Low-cost input proof/admission and browser/API checks
completed. Final helper build75174, input binary build67711, Rust regression77634, video
render77659 and strict clippy54177 completed; no heavy
six-row prover/verifier started. Default funded UI PID52864/exec33268 listens only
127.0.0.1:8877; log`.local/seller-bidder-demo-server-final-20261005.log`.
Previous UI72629/72510 was stopped before this restart (older20469/78615 had already
been stopped). Temporary public-only UI52934/17933 was stopped after its409 check.
Do not duplicate the default server. New ignored rehearsal namespaces/partial
checkpoints remain preserved; restart does not auto-resume them. Original
canonical private keys/journals/PCZT/witnesses/pads remain unchanged.
NU7 node29342 at112 and old6.3.0 node are unchanged. Automation stays removed.

Next: review the new front-to-back product story and its updated edited
video draft with the honest recorded-book transition.
The new110s edited video includes that entry and awaits human review; do not call
it a live run or real-time native proving. Second-machine/Linux performance/
reproduction remains pending available hardware.
Public testnet, independent operators and8/16/32 proofs are unfinished. Freeze
new cryptography; do not invent fresh funding/clearing/settlement from this
synthetic admission rehearsal. Repo stays private; no external messages,
release/deployment/submission.

## Earlier component milestones

**The accepted actual NU7 notes now form one six-output receipt PCZT.**
Fresh blinded-v2 REP3 delivery and fixed C++ winner proof feed the four refund
memos in the SAME unsigned PCZT, with all six certified recipients, actual
accepted nullifiers,15100-per-input amounts,30000 fee and canonical V3 openings
checked. Preparation9.118s; canonical node snapshot110 and all three original
Frozen journals unchanged. Eleven request/recipient/source/witness/certificate/
memo substitutions reject in3.566s; synthetic finalizer refuses actual scope.
At that preparation checkpoint, no native row proof, final settlement auction
proof, real custody signatures/reservation/broadcast was claimed; the later
completed run above supersedes its pending integration status. Existing synthetic verification stays strict.
Root/new SDK release builds and strict all-target clippy pass. Evidence:
`evidence/2026-10-05-nu7-funded-six-output-preparation.json`. The later canonical
run connects unchanged bindings/MPC and admission BEFORE actual DKG authorization.

**Four fresh NU7 deposits now produce an authenticated frozen book.**
Fresh local DKG escrow receives four actual15100zatoshi notes in a native NU7
transaction7d7fd7a4…56d1efe8 (18634bytes/five actions/25000fee). Zebra accepts it,
two controlled confirmations and all four canonical scanner paths pass at110.
New SDK decrypts actual memo ciphertexts and reuses existing claim-intent
validation: four client signatures, four input ZK proofs and12 delivered pairs
are reverified without reading maximum bids. Twelve acceptance receipts and
three final-book signatures are durable, issued after the declared real cutoff.
Same accepted root matches the client input book. Escrow admission reads FVK
only, not custody packages/master spending key. All actors remain on one host.
Thirteen actual ciphertext/claim/book/receiver/encoding substitutions reject;
eight certificate/root/authority substitutions reject without changing journals.
Legacy wallet17 tests, its examples2 tests, shared protocol12 tests and both
strict all-target clippy checks pass. Native circuit/verifier remain unchanged.
Off-chain source is shared to avoid an old/new SDK bip32 dependency conflict.
Root lock adds only the local package; no existing external resolution changes.
The separate SDK lock retains all327 original packages/checksums and adds179
for existing off-chain primitives. Selected-target inventories431/462 packages
have no missing license declaration. See
`evidence/2026-10-05-nu7-funded-admission.json`.
These accepted notes were Frozen/unspent at the admission checkpoint. They are
now all spent/finalized by the exact certified six-output transaction above.
Existing committed input proofs/shares use fixture tickets/terms on one host;
this is controlled regtest, not public testnet or a public ZK funding proof.

**Actual NU7 funding and DKG spending pass on a separate full node.**
Official Zebra7.0.0-rc.0 activates branch77190ad9 at height2 on its own regtest,
RPC29342; old6.3.0 state/port29332 are unchanged. Two matured native coinbase
notes are scanned at104, with canonical Merkle paths and416666666zatoshi total.
An actual bank note funds the local DKG escrow, then that escrow spends its
actual received note with its existing2-of-3 packages and no master spending key.
Both native transactions pass Zebra acceptance, two controlled confirmations,
separate exact recipient scans and source-note/balance reconciliation:
6bed5d9a…9a8b3f2c /6f761fa0…0e298a1; native proofs5.337s/5.002s.
The scanner refuses six branch/genesis/continuity/tree/encoding/cache changes;
altered actual witnesses reject before signing, three DKG package/readiness
changes reject before proving, and four reconciliation substitutions reject.
This is the necessary transfer migration gate, NOT the four-bid auction:
no new accepted book, MPC bindings, six-output memos or funded canonical E2E.
All DKG shares are on this host; the bank retains its ordinary spending key.
Node RPC/birthday are trusted, not an SPV proof; public testnet is not used.
See `evidence/2026-10-05-nu7-funded-dkg-transfer.json` and `docs/regtest.md`.

**One fixed PK now completes two distinct NU7 memo auctions.**
Opt-in warm full orchestration passes positive-price and zero-price books, using
one key generation and12 fresh row proofs. Verified clearing events:13.032s and
8.163s; settlement completes901.704s and823.613s after each request. Native/FROST,
normal fixed C++ proofs, independent SDK application replay and all six exact
recipient decryptions pass. Transaction731c2410…ad4e35d2 /26675832…871f754d.
One cold initialization122.447s: parameters92.015s, VK13.077s, PK17.355s. Six
sequential row services661.788s /583.039s; row create/verify are timed separately.
This is shared16GiB M1 paging, not a controlled speed comparison or2-minute claim.
Synthetic/local dealer only; no new funded admission, DKG or chain inclusion.
Driver89932/group89935/session6557 completed and were consumed; no duplicate run.
Fresh warm transaction public-only replay passes105.867s. Both books have four
own-memo openings and four substitutions passing17.021s/18.967s.
The original cold/cache paths and circuit/verifier/statements are preserved.
Mac2-way refuses before key allocation; larger Linux concurrency is unmeasured.
Guards10GiB start/4GiB disk/8GiB owned-group RSS/10%Mac memory stayed unchanged.
Sampled owned-group RSS3.10GiB, minimum disk4.63GiB/memory33%, maximum swap15.74GiB;
10-second observations are not true peak physical memory. See
`evidence/2026-10-05-warm-full-nu7-auctions.json`.
Only3093 rebuildable .rmeta/.rlib compiler cache files (3031265271bytes) were removed
after prior checks; binaries/source/proofs/keys/journals retained. Manifest stays
ignored in .local/warm-auction-compiler-cleanup-20261005.json.

**One current synthetic NU7 auction now includes the full six-output receipt flow.**
The run `.local/canonical-nu7-memos-20261005` completes at price50/sold3:
committed private bids → fresh blinded-v2 refund/winner delivery → four encrypted
receipt memos in the SAME six-output PCZT → six fresh shared18 native bindings →
normal fixed C++ MPC proof → four2-of-3 FROST spends → native proof/signatures →
six exact recipient decryptions. Transaction52555e259d4e6b8b8ddf029ae82ab297ad841a0de4316d0037dbd434f151ab3b,
21790bytes/30000zatoshi fee. Cold run1015.173s, native proof7.805s. Session69231
completed/consumed. Synthetic notes/local trusted dealer: no funded admission,
DKG, broadcast or chain inclusion is claimed. The older funded-v2 regtest is
separate. Initial13.79GiB headroom allowed this run without deleting artifacts;
unchanged10GiB start/4GiB disk/8GiB RSS/10% memory guards remained active.

**The browser/client/public bundle all refer to that exact transaction.**
Four recipients open only their own viewing identity; fixed flags+auction proof
and exact context/price/sold are checked before key access. Actual4 openings and
proof/wrong-key/trailing-tx/cross-run refusals pass in15.828s. Fixed public-only
settlement replay passes in99.323s; same-transaction HTTP replay101.076s. All8
native transaction/proof/action/layout substitutions reject in350.090s. Six more
portable missing-file/digest/external-path/uploaded-VK/symlink/cross-run cases reject;
producer-recomputed checksums do not replace fixed verifier trust. Same-run HTTP
four-ticket/15-boundary checks pass in12.475s; no secret fields are served.
A public-only checkout view starts without private identities, refuses own opening
without a key, and still rejects a corrupt proof. Its15 boundaries pass in3.284s.
The140386-byte public artifact is `demo/artifacts/canonical-nu7-v1`; no private
keys/pads/openings/witness/PCZT are included. `./demo.sh` starts the coherent view;
`./verify.sh demo/artifacts/canonical-nu7-v1` verifies the portable public bundle
with pinned tools prebuilt. No live browser bid admission/authentication is claimed.

**Same-circuit persistent PK is implemented and separately measured.**
A pipe-only shared18 worker warms one immutable key, then handles bounded jobs.
On the16GiB AppleM1, keygen111.984s; two new row0 proofs82.310s/92.506s,9504bytes
per proof; one key generation, zero signing approvals. The second fresh proof
replaces only row0 in a copied public bundle and passes the independent fixed
public verifier in103.105s. Requesting2-way concurrency on the Mac refuses
before key allocation.2/3/6 routing is compiled but not benchmarked on sufficient
Linux hardware. The original command retains its cold path; an opt-in full warm service now
passes both books above. No2-minute settlement or parallel speedup is measured.
Resource samples are every10s, not true peaks; warm max sampled RSS4064256KiB,
minimum disk7869779968bytes and memory34%. Short initial overlap with an independent
public replay is disclosed; that replay was stopped to serialize heavy work and
retained as `.local/canonical-nu7-public-attacks-20261005`, not a crypto failure.
Only rebuildable `target/release/deps/*.rmeta` compiler metadata was deleted
(1141971132bytes); all binaries/source/proofs/keys/journals were kept.

## Prior component records (historical; current result above is authoritative)

**The first local browser receipt/verifier UI works with actual proofs.**
`python3 scripts/serve_demo.py` is running at http://127.0.0.1:8877, PID63042,
exec session32486; log .local/browser-demo-20261005.log. This original server
was stopped for the public-verifier update. The current server is PID80820,
session72775, log .local/browser-demo-v3-20261005.log. Check this process/port
before launching another server. It is loopback-only and serves no repo/private
files. The API runs the existing actual recipient command and returns only own
status/refund plus public verification flags. No keys/pads/raw memos are served.
Four positive/zero winner/loser openings,15 HTTP/input refusals and two corrupt
proof refusals before reading the recipient key pass in9.743s. Native consensus
authentication is real; chain inclusion and six-output settlement are not claimed.
Browser verification displays "You won a unit" and refund50; tamper check visibly
rejects before key access. Narrow responsive layout also inspected; screenshots
are ignored under .local. The demo intentionally has no plaintext bid submission.
It uses the existing separate four-output synthetic memo transfers and says so
in the UI. All synthetic identities remain on one computer; choosing a ticket
is not bidder authentication. No external deployment/publication occurred.
Receipt/settlement statement equality also passes against the existing actual
zero-price six-row MPC statement, in .local/six-memo-zero-link-20261005; this is
a public statement-link check, not proof of the new memo transaction. Historical
non-memo NU7 pre-sign regression also passes all seven cases in0.105s.
No heavy prover is running. Next: registered client/admission/settlement views,
exact prepared NU7 proof resumption when10GiB initial disk headroom is available,
and advancing-chain memo delivery. Preserve historical funding/journals/keys.

**The public six-output settlement verifier is connected to the browser too.**
It snapshots the completed fresh-v2 price25 owned-regtest bundle and supplies
the original public authority from outside the bundle. The actual API check
passes in103.153s (native verifier103.076s): all three accepted-book endorsements,
six native row proofs, serialized transaction authorization and normal fixed C++
MPC proof match. No private opening/key/PCZT/funding journal is read. All copied
public artifacts have exact original source hashes; only public files are in the
snapshot .local/browser-settlement-9c7d0e1adbc1. Five path/authority/key/origin/token
API substitutions refuse before any verification. The result is fsynced locally
and the UI explicitly displays it as a previous verification, not a fresh run.
Browser state shows the correct funded-v2 transaction66b860cf…5b648. Earlier
UI-triggered verification completed in112.879s; its tab closed before subsequent
inspection, so a fresh API run validated the persisted/result rendering path.
Both completed jobs are consumed; current server remains idle. This historical
transaction has no receipt memos. Chain inclusion/current funding are not proved
by public replay, and no new signature/transaction/proving key was generated.

**Four receipt memos now belong to the same six-output NU7 prepared PCZT.**
Positive-price50/sold3 preparation completes in9.300s; price0/sold3 in7.937s.
Each runs fresh actual REP3 winner/refund delivery, normal fixed C++ verification,
four canonical ticket openings, native note encryption/decryption of all six
outputs and working-stack V3 commitment reconstruction. Seller/operator memos
remain empty. There are26 actual pre-sign rejections across both checkpoints,
including pad/ticket/proof/key/statement/refund/receiver/effects/PCZT changes and
refusal of missing six-row application proofs. Original source hashes match.
These are unsigned preparations: no six-row binding/native proof/signature,
funding/broadcast or full memo settlement claim. The implemented full path also
checks the receipt statement against the verified six-row context/price/sold
before FROST; that complete new run remains unexecuted below10GiB headroom.
Checkpoints: .local/six-memo-positive-20261005 / .local/six-memo-zero-20261005.
Initial attack session21755 failed because repeated fixed-verifier output labels
collided; diagnostics showed FileExistsError, not a cryptographic failure.
Unique output labels fixed the harness; fresh sessions5600/27546 complete all13
checks each in6.516s/5.997s. The failed original checkpoint is retained.
Do not change memos in historical signed/prepared funding checkpoints: IO
finalization erased dummy spend keys, while memo ciphertext changes affect the
signed effects. Existing journal reservations/signatures must not be reused for
changed effects. The new path builds memos before IO finalization instead.

Compact per-ticket receipts now pass actual native encrypted memo transport.
Zero-price session99068 is complete/consumed: four recipients use their own
viewing key only, native consensus proof/signatures and the fixed C++ flags
proof, then check ticket/root/pads/status/refund. All refunds100 still distinguish
three winners/one loser. 76 substitutions and 12 cross-recipient decryptions
reject, including native/AEAD encrypted memo mutation. Complete53.545s, native
proof5.923s, fee20000, 15478bytes. This is one fabricated input/four memo outputs,
not the auction's six-row settlement, live funding or network delivery. Full
NU7 six-row proof remains blocked below10GiB initial disk headroom; do not retry
unchanged. Positive-price transport also passes in55.756s, proof6.062s; all four
openings match50/50/50/100 refunds and three winners. Session30746 complete.
Total152 substitutions and24 cross-recipient decryptions reject. The original
two-output SDK native mode also passes in9.217s; session79437 complete. Four
input-tool tests, standalone SDK test, strict all-target release clippy and
both rustfmt checks pass; no lock/dependency changes.
The actual single-command recipient client passes four zero-price recipients
and six invalid-proof/key/statement/book/ticket checks in18.227s. Invalid proof
stops before copying the receiver key/decrypting a memo; session68979 complete.
Positive client session11532 completed with a resource guard refusal before
native decryption: host free disk fell below4GiB. This is not a ticket-proof
failure. Exact snapshots/keys/public proofs are preserved in
.local/positive-memo-client-20261005 and its log. Current disk is5.36GiB;
six-row10GiB gate remains blocked. Do not weaken guards.
Human reports a power interruption. Existing artifacts were preserved. A fresh
positive recipient-client check completed as session62101, log
.local/pos-memo-client-v2-20261005.log, after headroom returned above4GiB and
macOS reported79% memory free. Four positive-price recipients and all six client
attacks pass in14.353s; zero-price client18.227s. There are12 client substitutions
in total; invalid proof stops before reading/copying the receiver key/decrypting.
The new checkpoint preserves the interrupted/refused one and has no chain/
proving-key/broadcast operation. All jobs are complete/consumed; no new heavy
prover is running. Evidence: evidence/2026-10-05-encrypted-ticket-memos.json and
evidence/2026-10-05-recipient-memo-client.json.
Next: integrate proven receipt payloads into the six-output auction rather than
the separate transfer, then registered receiver/advancing-chain delivery. Full
NU7 native/application proof remains the primary dependency when10GiB headroom
returns; preserve prepared checkpoints, root lock and historical funded chain.
Initial session98696 completed with a harness output-label collision between
recipient verifications; its artifacts/log are preserved. Native transport and
the first recipient had passed. Fixed labels include the recipient directory.

**NU7 four-input/six-output unsigned preparation passes; full proof not run.**
The separate pinned SDK now consumes freshly verified blinded v2 delivery,
builds an unsigned six-action PCZT and exports canonical private note bytes.
The working stack reconstructs/checks all six actual V3 openings against the
new SDK commitments and auction-delivered amounts. Price 25/sold 3 preparation
passes in 8.365s; zero-price/sold 3 preparation with private winner flags passes
in 7.654s. No application binding proof, native proof, FROST signature or
transaction broadcast is claimed for these two prepared checkpoints.

Seven actual pre-sign attack checks pass: missing application proof, changed
amounts while conserving value, duplicate output index, substituted recipient,
changed effects identity, unknown private key fields and trailing PCZT bytes.
Upstream PCZT parsing accepted the extra byte; the adapter now requires an exact
canonical serialize/parse round-trip before interpreting a persisted request.
No transaction is emitted for any rejected checkpoint. Full NU7 application
binding/normal C++ verification before FROST and public-only replay are
implemented but remain **unverified** until a complete run succeeds.

Full six-row proof was not launched: the unchanged guard requires 10 GiB initial
headroom; .local/nu7-auction-shared-20261005.log records 6.94 GiB available and
5.95 GiB used macOS swap. Prepared artifacts, old chain/journals and root lock
remain intact. Do not repeat this full launch without capacity change. No
resource guard was weakened and no unrelated application/process was stopped.

**Nine committed-input MPC cases pass against the Rust sort oracle.** Session
63449 is completed/consumed. Tied cutoff, insufficient/exact demand, no qualified
bid, budget boundary, zero-price tie and three deterministic generated books each
pass four client proofs, 12 delivered pairs, actual REP3 auction proof and v2
private winning-flag proof, plus both fixed normal C++ verifiers. Price/sold and
all six delivered amounts/four winning bits match the separate synthetic Rust
oracle. Total 134.006s; all participant processes share this host. Oracle knows
fabricated bids; no malicious-security/native-funding claim. Evidence:
evidence/2026-10-05-committed-auction-cases.json.

**Per-ticket proof-authenticated winning status/refund receipts pass.** A
recipient's opening process uses its own expected ticket, two private pads only
(refund/status), and the public book/transcript. Fixed C++ verification is
mandatory before opening; opening CLI explicitly reports no proof verification
on its own. Zero-price receipts link the exact verified native settlement public
book/price/sold to the blinded flags transcript: four equal100 refunds still show
three winners/one loser. This completes in 104.391s including public native replay.
Positive-price tie and no-sale receipts pass in 2.114s/2.103s without native bundle
checks. All 72 ticket/root/mask/blinder/domain/extra-status changes reject. Receipts
are private proof openings, not operator signatures, new public disclosures,
funding/admission attestations or implemented network delivery. Sessions 70447
and 63120 are complete/consumed. Evidence:
evidence/2026-10-05-private-ticket-status-receipts.json.

All current jobs are complete/consumed; no heavy prover is running. Root lock is
unchanged. Separate NU7 inventory remains 295 packages with no missing license
metadata; direct serde1.0.229/libc0.2.190 pins reuse existing locked versions.
35 small-crate Rust tests, seven native tests, receipt opening regression,
23 Python tests, strict all-target release clippy for both workspaces, rustfmt
and Python compile checks pass. Full NU7 flow/replay remain capacity-blocked and
unverified; next priority is complete new-SDK six-row proof before wallet migration.


**Shared18 six-row native integration and public-only replay pass.** New blinded
v2 refunds, all six fresh native bindings, committed-input MPC, PCZT/2-of-3 FROST,
native consensus proof/signatures and six recipient decryptions pass with
synthetic funding. External complete run 709.426s; native binding key 108.790s,
six proof/initial verify stages 469.551s; no reused proofs. Public-only replay
103.945s also rejects trailing transaction bytes, changed binding signature and
action metadata. Session 5185 and replay 26086 are completed/consumed. Explicit
versioned metadata chooses one of two compiled circuits/fixed keys, never an
uploaded VK. Original working33 default/configuration remains unchanged. Sampled
RSS reaches 7,049,376 KiB, minimum free disk 9,596,473,344 bytes, maximum used swap
10,134,487,040 bytes; these are sampled, not controlled comparative measurements.
Evidence: evidence/2026-10-05-shared-six-row-integration.json.

**Fresh funded v2 settlement completes new deposits through exact confirmations.**
Session 10390 is completed/consumed; log
.local/regtest-shared-v2-auction4-20261005.log. New committed book
.local/mpc-auction4-20261005T103810Z has price 25/sold 3. Four fresh deposits into a
new local DKG escrow are full-node accepted with two confirmations at height 116;
all 12 signed receipts and three final-book signatures match the actual cutoff.
Deposit gate 32826 completed/consumed. Settlement uses explicit shared18 with
10 GiB initial headroom and unchanged 4 GiB disk/8 GiB RSS/10% macOS guards.
Six fresh row proofs (no cache), actual committed MPC, certified root/recipients,
PCZT, four DKG threshold spends, native consensus proof/signatures, full-node
broadcast acceptance, two confirmations, six exact decrypted receipts and three
Finalized journals pass. Price 25/sold 3, fee 30,000, 21,790 bytes, txid
66b860cf45a173ad03dd7b7112ff731b12ef15e62857a096c1488979add5b648.
Construction 819.180s; application 800.454s, native proof 8.148s; current owned
chain height 118. Maximum-bid list, client spending keys and escrow master ASK are
not read/reconstructed. All participants still share one host. No public-chain
finality or mainnet claim. Sampled max RSS 4,054,224 KiB, min disk 7,368,859,648
bytes, max used swap 11,549,739,581 bytes; no controlled comparative benchmark.
Evidence: evidence/2026-10-05-fresh-funded-blinded-v2.json.

Public-only certified replay passes in 100.871s; session 24305 is completed/consumed,
log .local/regtest-shared-v2-public-20261005.log. Ten certificate/book/recipient/
unknown layout/version/uploaded VK changes reject. Zero-price native consumer connects
the v2 flags circuit, independently verifies its normal C++ proof, freshly
re-derives the private opening from the verified transcript/pads and checks
winner count/refund/conservation. Three selected Rust integration tests and 21
Python tests pass. Actual delivery-only zero-price consumption also passes at
price 0/sold 3; session 90668 is complete. **Full zero-price synthetic native gate
passes**, session 17906 completed/consumed. v2 private winner flags, six fresh
shared18 bindings, committed MPC, PCZT, four FROST spends, native proof/signatures
and all six decryptions pass. Price 0/sold 3, one zero-valued seller output,
fee 30,000, 21,790 bytes; external total 742.670s (application 725.029s,
native proof 8.191s). This historical run alone did not deliver bidder receipts. New local proof-backed
per-ticket receipts are recorded above; network delivery remains unfinished.
No chain funding/broadcast. Evidence: evidence/2026-10-05-zero-price-native-v2.json.
Zero-price public-only replay passes in 103.898s. Seven transaction/action/layout
changes reject, including selecting the other valid compiled circuit/VK
(102.211s). Session 17103 is completed/consumed. Original working33 historical
public regression also passes in 104.549s; six original row proofs, exact native
transaction/signatures and normal C++ auction proof pass, with no private inputs
or new chain operation. Session 99455 completed/consumed. The original default
and trusted configuration remain unchanged; evidence is appended to the shared
six-row record. Resource-guarded synthetic
driver: scripts/run_integrated_gate.py. Native build 52352/74249 is complete;
35 selected small-crate Rust tests, six native unit tests, two layout tests and
latest strict release clippy also pass. Test session 20806 is completed/consumed.

**A nested-process interruption bug is reproduced and fixed.** Synthetic old
behavior leaves a new-session child alive after its resource guard aborts.
Only that owned test child is cleaned up. The guard now execs its command inside
a fresh group with a marker matching its actual pgid; nested delivery/settlement/
auction MPC runners inherit that group, while standalone runners retain their
own child groups. Stage cleanup signals only its child or its owned group.
Parent/time-wrapper/grandchild interruption and unrelated-parent survival tests
pass. All 23 Python tests and an actual guarded delivery_flags4 REP3 run pass
(7.762s); normal C++ verifier also passes. Session 38242 is completed/consumed.
Evidence: evidence/2026-10-05-nested-mpc-resource-abort.json. No cryptographic
predicate, resource limit, wallet journal or confirmed chain transaction changed.

**Shared18 complete-cache partial-Prepare recovery passes under the new guard.**
Session 38702 completed/consumed; log .local/shared-v2-prepared-20261005.log.
Guarded process 232.798s, six cached proofs reverified in 101.419s, zero proving-key
generation, exact original txid/effects, native/MPC/DKG FROST and six openings pass.
Two Prepared copies have zero new/duplicate writes; one Frozen copy receives
one Prepare. Cancellation-purpose substitution rejects before proving/signing.
Original PCZT/proofs and Finalized journals remain unchanged. Historical notes
are already spent; no RPC, broadcast, mining or new chain-funding claim.
Evidence: evidence/2026-10-05-shared-layout-prepared-recovery.json.
No recorded jobs are active. This turn started from pushed checkpoint 9f7ef8a;
new NU7 adapter/preparation and nine committed-MPC case changes are recorded in
8d0863d and 9115d10. The per-ticket receipt source/evidence and this status file
are committed together; git HEAD is authoritative for the current checkpoint.
Next priority is the full six-output NU7 proof when at least10 GiB disk headroom
is available. Until then, advance private receipt transport/client integration;
do not repeat an unchanged capacity refusal or weaken resource guards.

Independent plaintext Rust/Noir ACVM regression passes 77 valid and seven invalid
generated cases plus rejection of a deliberately wrong reference result. This
does not generate MPC/ZK proofs or read private live bids. Evidence:
evidence/2026-10-05-cross-noir-reference.json. All build/check/reference sessions
61132/73271/96182/84241/22039/77530 are completed/consumed. Earlier diagnostic-only
harness failure 47317 is retained; all cases were rerun after correcting the
pinned ACVM constraint-error predicate.

**A concrete delivery privacy flaw is reproduced and the v2 fix passes actual
REP3, independent C++ verification and offline native MPC/FROST recovery.**
Legacy delivery4 deterministically commits each mask; public price/budget let
a transcript reader hash the two possible refund masks. A public-only audit
recovers all four historical local DKG refund candidates, and four winner bits
from the initially drafted unblinded flag variant. Fresh masks alone do not hide
these candidates. No full maximum-bid recovery or external publication/leak is
asserted; the final public settlement bundle excludes delivery transcripts.
Withdraw v1 transcript privacy claims and preserve historical proof reproduction.

New delivery_private4 and delivery_flags4 add fresh independent uniform private
commitment blinders, with separate versioned domains/pads for refunds and winner
bits. Nonzero refund and zero-price flag REP3 gates pass; normal C++ trusted keys
match and changed price/root/cipher/flags reject. Four input-tool and three Noir
tests pass. The 16-small-blinder dictionary check is a regression, not an exhaustive
hiding proof. New native construction requires v2. Native delivery-only consumption
passes; no native transaction/signatures in that smaller gate. Actual secret share/
witness writes now inherit umask 077 in 0700 MPC directories; the local parent is
restricted to 0700, protecting historical fixture contents without changing bytes.

The exact offline copied partial-Prepare DKG gate now also passes with v2 delivery:
235.147s total, application 218.865s, native proof 7.941s. All six original row
proofs reverify (102.196s); **no proving key is generated** for complete-cache
recovery. Original trusted working33 circuit and VK remain unchanged. Same txid/
effects, four DKG threshold spends, native proof/signatures, six exact openings and
certified MPC/native checks pass. Two Prepared copies receive zero duplicate
writes; the Frozen copy receives one Prepare. Original source/proofs/journals are
unchanged. Historical notes are already spent: no RPC, broadcast or new funded
auction. Sampled free disk stays above 13,016,420,352 bytes during this gate.
Changed complete cached proof also rejects before key preparation/signatures or
journal writes (106.276s); no regeneration fallback. Fresh public-only certified
replay also passes (107.491s): exact native proof/signatures, all six original
row proofs, normal C++ settlement proof and all-three signed final-book receipts,
without delivery pads/blinders/shares/decoded amounts/private openings. Seven
certificate/authority/book/recipient substitutions reject. Session 53311 is
completed/consumed; log .local/prepared-v2-public-20261005.log. Positive native
recovery 1647 and negative complete-proof gate 3821 are completed/consumed too.
Eight selected Rust tests, 18 Python tests, rustfmt and strict selected release
all-target clippy pass. No recorded jobs are active.
At that earlier checkpoint, fresh funded v2, full native zero-price and
public-testnet NU7 were unfinished. The current outcome above supersedes it.
Evidence: evidence/2026-10-05-blinded-delivery-and-dictionary-attack.json.
Normal native construction now preflights 16 GiB disk; signed reconciliation is
unchanged. Runtime guards retain 4 GiB disk/8 GiB root RSS/10% macOS headroom.


**The separate shared18 NU7-output binding proof and public-only replay pass.**
The exact new-SDK synthetic V3 output bytes now produce a trusted shared-layout
Halo2 proof linked to the actual serialized NU7 native cmx/index. Proof 9,504
bytes; key 115.076s, proof+initial verify 104.009s, complete run 221.541s. Altered
hash/cmx/index rejects. Six MockProver runs reject amount, valid recipient point
and public-input substitutions (57.485s). Shared layout uses 18 advice columns
instead of 33, same canonical byte/integer/native/copy constraints and K17/degree9.
The original configuration fingerprint remains exactly
22fe5ec710bd191052c1b208e84f70c84f3612a2b8d4aae3e57654b1fa1bd4cd.
Existing funded statements continue on that original trusted circuit/VK.

Public replay (107.893s) freezes only four public files, no note openings, private
row salts, masks, keys or PCZT. The new SDK verifies native proof/signatures and
actual cmx/index mapping; the separate explicit shared verifier independently
verifies the fixed row circuit and rejects altered hash/cmx/index/proof. Changed
cmx/index/txid mapping rejects; wrong layout and extra private public-row fields
reject. This is a single synthetic-context output, not certified auction/chain
funding or public broadcast. It does not migrate the full wallet/auction SDK.

Completed/consumed: shared mock 45752, shared proof 78357, public replay 17290;
logs .local/nu7-shared-{mock,proof,replay}-20261005.log. Max sampled proof RSS
1,291,536 KiB; min free disk 4,963,872,768 bytes; max sampled used swap
12,309,558,722 bytes. Host workload changed (reported memory headroom 34% -> 70%):
no controlled memory/latency improvement claim. The measured swap growth during
key/proving corroborates transient disk loss. All normal runtime guards stayed;
no other host work was stopped. Evidence: evidence/2026-10-05-shared-binding-layout.json.
Original funded transaction plus all six original working33 row proofs and normal
C++ auction proof also reverify after the layout refactor (111.190s, public-only,
no fresh certificate check/chain inclusion). Regression 58048 completed/consumed.
Layout configuration regression and strict selected/standalone all-target clippy
pass. Next: preserve old reproduction, connect full NU7 auction/wallet path and
live-chain freshness. Zero-price winner-delivery circuit checks are also underway.

The original working33 new-SDK output binding experiment hit the 4 GiB disk guard
twice during key/proof generation: 210.430s at two Rayon threads, then 341.850s
at one thread despite 10 GiB initial disk/59% reported memory headroom. Both owned
provers are stopped/consumed (2525/11285); no completed new-output row proof and
no complete public binding replay. Logs .local/nu7-output-binding.log and
.local/nu7-output-binding-singlethread.log; private exports/source are preserved.
Evidence: evidence/2026-10-05-nu7-binding-resource-stop.json. Do not repeat the same
heavy keygen without more capacity; a future original-layout gate needs at least 16 GiB
initial disk headroom and all runtime guards. No proof/guard/branch downgrade.

Implemented separate byte export/public-only mapping replay and existing-circuit
binding commands. Selected/root and standalone all-target release clippy pass.
Native NU7 proof/signatures/decryptions/export pass (12.042s). Public tx-cmx/index
matching and nine public/private malformed mapping/opening cases reject; this
preflight is not an application ZK proof. Evidence:
evidence/2026-10-05-nu7-byte-input-checks.json. Original funded chain/journals are
unchanged. The separate shared-layout result is above; full NU7 application migration remains.

Static native layout profile (no private input/SRS/proof allocation): K17, 33
advice columns, degree 9 and 8x extended domain (2^20 rows). This is not a memory
measurement. Sampled post-stop macOS swap is about 6 GiB; free disk returned to
11 GiB while .local remains 42 MiB. Transient swap is the likely disk-loss cause,
but no contemporaneous swap series proves exact attribution. Resource sampling
now records used swap; five resource tests pass. Keep existing trusted layout
and investigate a separately identified shared-column variant. Evidence:
evidence/2026-10-05-native-binding-static-profile.json.

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

Heartbeat darkcall was deleted at the human's request. Continue on direct human
requests; do not recreate recurring scheduling. Preserve evidence and coherent commits.
