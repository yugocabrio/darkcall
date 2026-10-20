# DarkCall task queue

## Oct 18–19 handoff to Claude Code (overrides older queues)

Deadline: Oct 28 23:59 UTC. Keep the proof/security model. No new cryptography,
scaling or proving-speed research before submission.

Done:
- [x] Restore the environment (NU7 node, bank rescan, root binaries incl. output-binding bins).
- [x] Same-book settlement of browser auctions (CLI `54941ffd…`, HTTP `0e77bc23…`, browser `3fee2c7c…`, `ac06fc31…`).
- [x] Fund before Clear: fund/settle phases, `/api/rehearsal/fund`, six-step UI; harness `94a093e9…`.
- [x] Six-step browser E2E in one page (`e3ffa436…`): receipts WON/LOST, tamper, 390 px; headline bundle v2.
- [x] Judge journey polish: landing contrast, sticky steps, sealed slots, next-action focus, honest labels.
- [x] Reproducible 2-minute video from one real auction (`demo/media/v2`, capture tool, renderer).
- [x] Clean-room Linux container verifier; two portability defects fixed (hidden `witness_point.rs`, diff abbrev).
- [x] Hosting plan for judges (`docs/hosting-plan.md`); nothing deployed.
- [x] Static evidence page `docs/index.html` (tier A), built from records; not deployed.
- [x] Strength-led redesign (Oct 19): claims + real settled card on the landing, comparison with
      open / commit–reveal / trusted designs, measured numbers; paper/ink/gold with self-hosted
      IBM Plex; evidence page, video overlays, README and submission draft aligned.
- [x] Public Zcash testnet settlement (Oct 19, approved by the author): deposits `d573a651…`,
      settlement `8e6da99b…`, public check 98 s, tamper refused; bundle `testnet-auction-nu7-v1`.
- [x] Final video on public testnet from the browser (Oct 19): auction `4dde6fcb…`, deposits
      `af6cf7cf…` (block 4,518,254), settlement `be1157c8…` (block 4,518,315), receipts 79 / 100,
      tamper refused; bundle `testnet-auction-nu7-v2`; captions drawn in IBM Plex by the browser.
- [x] Landing, "See a settled auction", evidence page and README all show that one run.
- [x] How it works: four stages, "Follow one auction" figure (share ownership 1/2/3 per
      operator), six steps with what stays private / what is proven, Halo2 binding diagram.
- [x] Technique-first copy (Oct 19): hero names multi-party computation (REP3), multi-party ZK
      proving (coNoir) and proof-bound settlement (Halo2); "Why multi-party ZK proving" vs a single
      prover; shares shown as A①③ / B②① / C③②; BN254↔Pallas binding with what checks what;
      SHA vs Poseidon2 numbers scoped to the coNoir step. Evidence page phone layout fixed (the
      walkthrough overflowed by 302 px at 390 px; its rules now travel with the copied section).
- [x] Motive-first story (Oct 19): hero lead opens with what other designs reveal (willingness to pay);
      Split → Prove → Pay as one visual flow; the six payouts named (4 refunds, seller, fee) wherever
      "six" first appears; binding section led by "Proving the payment is the other half", labelled
      multi-party ZK proving × Halo2 output binding, with the curve bridge, the check split and the
      performance note kept apart. README image recaptured from the landing.
- [x] First-read gaps (Oct 19): why the price is 21 (demo bids 77/61/43/21, top three win), what the
      15,100 deposit is (100 budget + 15,000 processing fee covering the 30,000 network fee and the
      operator fee), demo-sized amounts, price and units sold both public, three signatures on the
      book vs any two FROST signers for the payout, clearing 9 s vs settlement 19 min, comparison
      heading narrowed to commit–reveal. Testnet txids link to ZecBlock (third-party explorer;
      both resolve, v6, blocks match the record) on the landing, evidence page and README.
- [x] Money and contrast (Oct 19): the walkthrough's settle card adds the public 30,000 network fee so
      six outputs + fee = 4 × 15,100; the Settle caption explains winner refunds (100 − 21 = 79), the
      loser's full 100 and the seller's 63; completed walkthrough stages stay at 82% opacity instead
      of 50%; hero lead scoped to commit–reveal; payout wording and operator knowledge made exact;
      duplicate trust caveat removed. Hero CTA order left unchanged
      (author chose technique first; at 1366×657 the CTA starts at 691 px).
- [x] Section order (Oct 19): the payment-binding section now follows "Why multi-party ZK proving",
      so the two differentiators read back to back before the walkthrough and the six steps.
- [x] Mid-width and guarantee scope (Oct 19): the landing hero stacks below 1000 px (at 820 px the
      flow cards had shrunk to about 80 px); example bids labelled; the binding lead says colluding
      signers could still move the escrow but such a payout fails verification; verification uses
      operator admission keys obtained independently of the bundle (site and trust model); evidence
      page scope note lists price and units sold; six-step heading renamed "A closer look at the
      protocol". Checked at 390, 820, 1024 and 1440 px.
- [x] Judge time (Oct 19): "See a settled auction" is now the first, dark button; "Run an auction"
      states about 30 min on testnet (20 on regtest). Fund and Settle show measured testnet times
      (about 10 and 16-19 min) instead of regtest ones; the hero settlement txid links to ZecBlock
      when the recorded run is on testnet; comparison row reads "Anyone can verify payouts match the
      result", the column "decryption committee", and the caption notes other ZK/MPC auctions.
- [x] Cleanup (Oct 19): removed the superseded Oct 5 video draft (docs/demo-video.md, its renderer,
      storyboard and stills 01-19); the current video is demo/media/v2. Ignored scratch under
      .local (old runs, logs, drafts) moved out; demo.sh's recorded runs, both banks and the regtest
      node data kept. Recorded-mode HTTP checks pass on testnet and regtest after the cleanup.
- [x] Way back (Oct 19): "← Back to overview" beside the mode switch returns from either mode to the
      hero and How it works without a reload; a running auction stays and "Run an auction" reopens it.
- [x] Bid-count scaling (Oct 21): measured N-bid variants (4/8/16 gate counts; 4 and 8 joint clearing
      proofs run and verified; 16 estimated). README, judge guide and draft say four is the circuit
      size, not a speed limit. Next if needed: a sorting network beyond a few dozen bids.
- [x] CI (Oct 21): GitHub Actions runs 34 Noir, 66 Rust (release) and 28 Python tests on every push;
      all pass locally. Own-wallet deposits: format allows ZIP-321 payment, but no consumer wallet
      sends NU7 testnet transactions yet (Oct 15 Arborist call); recorded in the judge guide.
- [x] One site (Oct 21): the static evidence page (docs/index.html, docs/site, build_evidence_site.py)
      is retired; the landing at / and the live demo at /demo/settled and /demo/run are the site,
      with shareable paths and working browser history.
- [x] Hosted live demo prepared (Oct 21): serve_demo.py hosted mode, docker/demo (image, Caddy TLS,
      compose), scripts/hosted_local_set.py (11 recorded-run files + testnet bank; the recorded-mode
      HTTP check passes with only that as .local). See docs/hosting-plan.md.
- [ ] Deploy the hosted demo: waiting on the author's server and SSH access.
- [x] Review fixes: job timeouts stop the whole process tree before the lock is released;
      "Start another auction" cannot drop ticket capabilities mid-job or silently; the bid
      helper is described as the bidder's own client; colluding-signer caveat stated.
- [x] Name kept: "DarkCall", descriptor "Verifiable sealed-bid auctions on Zcash".
- [x] Descriptor (Oct 19): "Private, verifiable sealed-bid auctions on Zcash". "Sealed-bid" alone
      usually means bids opened at the close, so "private" now states the never-opened half.
- [x] Clean-room container covers all seven bundles (Oct 19, strict tamper refusal): first
      with the 66988f5 image and bundles mounted read-only, then with a fresh image built
      from `a7f67bd` by `scripts/docker_verify.sh`.
- [ ] Optional (about a day): four bidder wallets each funding their own deposit instead of one
      local test bank, to remove the "one wallet funds all four" limit.

Open:
- [x] Container re-run via `scripts/docker_verify.sh`: all four committed bundles verified, strict tamper refusal.
- [ ] Final consistency and adversarial review; final report to the author.
- [ ] Optional, needs approval: tier-A static viewer or tier-B public verifier (see hosting plan).

### Human actions before Oct 28 23:59 UTC
- Make the repository public (rule 3) and enable GitHub Pages from `main` / `/docs`
  (the author will do both).
- Submit in **Private Markets** using `docs/submission-draft.md`; one submission per
  account, so Zatomic needs a separate account if both are entered.
- The 2-minute video is `demo/media/v2/darkcall-demo-v2.mp4` (testnet run); a narrated
  live recording is optional (`docs/demo.md` → "Capture-ready demo plan").
- Docker Desktop was restarted on Oct 19 with approval; the other project's node
  was restored with `docker start`.

Latest human instruction: start development, commit and push regularly, continue
autonomously, incorporate later human/ChatGPT reviews.

## Latest human priorities (Oct5; override the older queue)

The15-minute automation is deleted at the human's request. Continue on direct
requests. Converge on a Grand Prize submission; preserve exactly the current
proof/security model and avoid unrelated recovery/wallet/cryptographic work.
After the funded canonical milestone, freeze new cryptography. Focus on review
packaging, reproduction and presentation, followed by measured performance and
bug fixes. Second-machine/Linux results require an actual available host.
Latest frontend priority supersedes further review packaging: implement the
seller → bidder → auction entry loop, preserving all existing verification and
native security conditions. Fresh client commitments/shares/acceptance can be
real while settlement transitions explicitly to a DIFFERENT recorded book.
The localhost helper sees typed demo bids; funding is explicitly synthetic.

Latest continuity priority: the newly accepted book now runs the EXISTING REP3
clearing circuit and opens its own result. Native payment still uses a separate
recorded auction. Do not replace fresh random tickets/commitments with an implied
same-root preset or redesign cryptography.

- [x] Latest visual priority: separate **Try DarkCall / Proof it settles** mode
  cards, compact navigation, Create/Bid/Clear/Result progress, paired fresh price
  and own result, expandable bids/acceptances and shorter visible copy. Existing
  request payloads, proof gates, recorded handlers, circuits and authority remain
  unchanged. Actual new book: price43/sold2,8.284s, own entitlement57 and tamper
  refusal; recorded winner/loser/public settlement pass.390px does not overflow.
- [x] Refresh all seven video stills to the polished UI and keep the explicit
  different-recorded-auction switch.110s/11 scenes; edited stills, not live proving.
- [ ] Human review of the two-mode design and video. Independent second-machine
  reproduction remains unavailable on this host; no new cryptography queued.

- [x] Same-book fresh browser clearing: canonical reindex of unchanged share
  values, exact signed root/conditions/tickets, actual3-party REP3 flags proof,
  fixed C++ verification and per-ticket opening after proof/certificate gates.
  Positive price43/2 sold and zero-price3 sold pass, four own results each,
  capability/statement/certificate/proof refusals and idempotent clearing.
  Browser WON/refund entitlement57, tamper and mobile390px pass.
- [x] Update the edited video to110s/10 scenes: fresh same-book result before
  explicit switch to a DIFFERENT funded native example. Visually checked.
- [ ] Human review of current same-book story and video; polish the visible
  interaction. Independent host reproduction/performance remains host-dependent.
  Fresh wallet/native payment and production private admission are not complete.

- [x] Actual seller create/share and bidder input proof/share/receipt entry, plus
  cutoff/four-ticket final-book certification. Four fresh input proofs/12 pair
  checks/12 receipt signatures/3 endorsements;17 rejection cases, pre-admission
  proof/share/context substitutions leave journals unchanged. Actual browser
  create/seal/copy, local join, mobile390px and recorded receipt/tamper pass.
  Earlier271a323 entry did not run clearing; the checked item above now connects
  actual same-book MPC. Native funding/signing/settlement remain separate.
- [x] Adapt the video/storyboard to include seller → bidder entry and explicit
  different-recorded-book transition. Latest110s/9-scene edited draft shows three
  native entry screens, a switch card and existing recorded verification/trust.
  Footers distinguish offline/synthetic entry from recorded funded transaction;
  no live-auction/proof-timing claim. Prior result-only draft is preserved.
- [ ] Human product-loop review; real in-browser/private transport admission,
  wallet funding, idempotent bid/restart recovery and fresh settlement hookup
  remain separate implementation work, not claimed by this local rehearsal.

- [x] Produce a committed-source review kit, validate its exclusions and public
  links, and start its extracted public-only browser with no private identities.
  - Revision61337e8:411 committed source files,1309909-byte archive; same-host
    extraction/public-only page passes. No Git/runtime/dependency/key export.
    Missing own identity refuses409; missing compiled tools report incomplete
    prerequisites. This is not second-machine or cryptographic reproduction.
- [ ] Reproduce the fixed public verifier on a second machine. Prepare exact
  tool/patch/CRS instructions and preflight; do not call a same-Mac fresh directory
  an independent reproduction. Optional human hardware question is pending.
- [x] Simplify judge-facing UI without backend/API/security changes. Winner/loser,
  tamper and public settlement pass on the same canonical funded transaction;
  actual verification controls badges. Technical/trust details collapse;390px
  mobile has no horizontal overflow. No unconditional “nobody sees” claim.
- [x] Render a short captioned edited demo draft and review copy with exact
  guarantees/limits.110s/1920×1080, six verified UI stills plus two scope cards;
  source storyboard/renderer tracked, MP4 ignored. This is not real-time proving.
- [ ] Human review of the demo draft; narration/continuous capture if desired,
  final submission assets. No publication or contest submission performed.

- [x] One canonical actual current-NU7/v2 funded run: authenticated4 deposits,
  signed final root/recipients, blinded REP3 delivery, four encrypted receipt memos
  in the SAME six-output transaction, fresh bindings/MPC, existing DKG FROST,
  Zebra acceptance,2 controlled confirmations,6 exact scans/3 Finalized journals.
  Same tx47019e02…a211938 in public replay, four own clients and browser.
  - [x] Separate official NU7 full node and actual-note/DKG transfer migration.
  - [x] Fresh actual four-deposit certified admission:13 claim/8 authority refusals.
  - [x] Actual accepted notes → same6-output preparation:9.118s/11 substitutions.
  - [x] Exact prepared PCZT recovery after disk guard stop:1847.653s/one thread;
    original190.563s attempt preserved, no replacement transaction or weaker checks.
  - [x] Same-run public-only replay112.431s,8 public substitutions13.929s.
  - [x] Four own-FVK memo clients plus6 substitutions22.247s, invalid public proofs
    and certificate reject BEFORE own identity read. No spending keys in client.
  - [x] Default funded browser,4 receipts/15 HTTP gates17.990s, public-only view
    without identities1.169s, full HTTP settlement110.112s, separate API boundaries.
  - Fixture tickets/terms/proofs/shares remain on one host. Not public testnet,
    malicious MPC, public ZK funding/inclusion or live browser admission.
- [x] One coherent recorded browser story using that exact same run/bundle.
  - Default `./demo.sh` uses certified actual NU7 run; `--synthetic` preserves the
    old synthetic story. Public authority is configured separately from bundle.
- [x] One-command demo and fail-closed public verification; fixed verifiers,
  portable public paths, no private material or bundle-discovered authority.
- [x] Focused same-funded-bundle review:8 native/certificate/authority refusals,
  6 own-client substitutions,7 portable path/key/authority/context refusals,
  15 HTTP boundaries and5 settlement API boundaries. No new exploit found here;
  keep one-host/regtest/semi-honest/attested-funding limits explicit.
- [ ] Profile cold keygen/prove/verify separately; warm the same PK across jobs,
  implement bounded row workers without relaxing verification. No6-way proving on
  this16GiB Mac.1/2/3/6 concurrency comparisons require sufficient measured hardware;
  a2-minute target is not a measured claim.
  - Pipe-only same-PK jobs pass: key111.984s, fresh proofs82.310s/92.506s.
    Independent fixed public verification of a warm-produced proof passes.
    Cold full-run key108.596s/six proof-services667.790s. Mac2-way request
    refuses before key allocation. Opt-in full auction orchestration now
    passes two distinct books with ONE PK and12 fresh row proofs. Key122.447s
    (params92.015/VK13.077/PK17.355); clearing13.032s/8.163s, full settlement
    901.704s/823.613s, row services661.788s/583.039s. No2-minute claim. Larger-host
    concurrency and8/16/32-bid proof shapes remain unimplemented/unmeasured.
- [ ] Scaling measurements4/8/16/32 without weakening guarantees; exact failures
  are useful results. Submission draft now: product, diagram, threat model,
  verified/unverified table,90–120s script, verifier and benchmark/license notes.

## Schedule

| Window (2026, UTC dates) | Outcome |
| --- | --- |
| Oct 4–6 | Four-bid collaborative proof, native threshold spend, smallest output-binding proof |
| Oct 7–12 | Four-bid funded end-to-end auction and refunds |
| Oct 13–19 | Cancellation, malformed inputs, omissions, transaction substitution, verification |
| Oct 20–24 | UI, reproduction, demo video, measured scale |
| Oct 25–28 | Feature freeze, bug fixes and submission package |

## Priority 0 · smallest technical gates

- [x] P0.1 Pin coNoir/Nargo/Barretenberg; compile a four-bid circuit.
- [x] P0.2 Enter actual REP3 shares, jointly generate a ZK proof, verify using the normal verifier.
  - Four separate synthetic input files, joint witness/proof and standalone coNoir
    verification passed. Matching C++ Barretenberg verification also passed, with
    the trusted key matching exactly and altered price/root rejected.
- [x] P0.3 Validate shares against bid commitments; reject bad shares and duplicate tickets.
  - Client ZK proofs bind bid openings to all three share-pair hashes. All 12
    delivered pairs and the committed MPC root verified in the synthetic gate;
    changed shares, price and root were rejected. Four actual-regtest deposits now
    repeat all client/pair checks before authenticated durable acceptance.
- [x] P0.4 Pin the Ironwood/PCZT/FROST stack; offline native transaction and signature tests.
  - Source pinned; upstream's 3 v6 sighash regression tests pass. Own transaction
    construction, native proofs, threshold signatures, extraction and 6 recipient
    decryptions passed. Both ordinary and zero seller/operator output cases passed.
- [ ] P0.5 Receive testnet funds, sign 2-of-3, broadcast and decrypt the recipient note.
  - Actual isolated regtest equivalent passes: matured native funding, canonical
    witnesses, 2-of-3 FROST, full-node acceptance, two confirmations and exact
    recipient scan. Standard public testnet uses upstream NU7 at 4465026; our pinned native
    stack lacks branch 77190ad9. Isolated new-SDK NU7 PCZT/FROST/proof/decryption, read-only native frontier
    reconstruction and old native transaction replay now pass. Auction/native
    binding migration and real public funding/spending remain; preserve the old gate.
    This gate is a transfer, not the funded auction; trusted-dealer master key is
    retained. A second valid spend of the same note is rejected by the node.
- [x] P0.6 Prove one private settlement row binds to a native note commitment.
  - Actual Halo2 proof (4,928 bytes) verified against a native Ironwood output;
    changed row hash/cmx and amount/recipient checks rejected tampering.
    Prototype Pallas hash is not connected to the BN254 auction plan. Classical
    binding only; ZIP 2005 rseed derivation/ciphertext correctness not constrained.
- [x] P0.7 Four-bid amount/recipient/effects/signature/transaction substitution rejects; measured runtime and sampled resources.
  - Actual funded DKG auction and public-only replay passed; 10-second root RSS,
    disk and reported macOS headroom sampled. Resumed/cold/shared-host timing
    scopes are explicit; these are not true peak-memory or independent benchmarks.
- [x] P0.8 Confirm the supported four-input/six-output shape, bytes and fee.
  - Four-input/six-output regular and two-zero-row transactions proved, verified,
    and decrypted: 21,790 bytes, standard fee 30,000. Other supported counts remain.

## Priority 1 · integration

- [x] Replace retained escrow master signing key with local DKG-generated 2-of-3 shares.
  - Bank/client spending keys are normal owned wallets; only the shared escrow
    setup is in scope. Local DKG/FVK, all three two-party quorums, durable
    packages and actual four-deposit certified admission pass. Full funded
    native DKG settlement now passes with four actual funded inputs, six exact
    recipient wallets, full-node confirmations and three finalized journals.
    Exact built-PCZT recovery reused two verified rows after controlled process
    interruption. All shares remain local; independent operator hosts are separate.
- [x] Integrate current NU7 auction on isolated regtest after compatibility gates.
  - Canonical funded tx47019e02…a211938 completes the four actual accepted notes,
    signed admission, blinded-v2 memo delivery, six shared18 proofs, fixed MPC,
    existing DKG, full-node acceptance, confirmations and exact wallet scans.
    Root/SDK locks, byte interop, historical working33 verifier and original
    NU6.3 chain remain preserved. The old working33 NU7 proof attempts were
    resource-limited; the explicit shared18 variant is demonstrated. Public
    testnet and general advancing-chain freshness/recovery remain separate gaps.

- [x] Canonical four-ticket conditions/IDs, certified accepted root and six settlement rows.
- [x] Initial oracle and circuit fixtures agree on ties, insufficient demand and empty auctions.
- [x] Generated cross-language plaintext auction-rule equivalence.
  - 77 valid and seven invalid Rust sort/Noir ACVM cases plus a deliberately
    wrong expected result pass. Zero price, all active masks, ties, unsigned full
    ticket sorting and money bounds are covered. No MPC or ZK proof is generated.
- [x] Initial generated-case equivalence with actual committed-input MPC proofs.
  - Nine synthetic four-active cases each pass client/pair checks, actual REP3
    auction/v2 flags proofs, both normal C++ verifiers and Rust amount/status
    comparison. Larger/inactive books remain separately unfinished.
- [x] Shared four-bid/six-output commitment encoding with constrained Poseidon/native hash links.
  - Canonical 224-byte SHA-256 format, linked Halo2 SHA and actual native row proof
    passed. Six-row Noir plan circuit compiles/tests pass. Complete MPC hash proof
    passed on the same private row using ARM AES-enabled coNoir (574.499s proof
    stage). Normal C++ verification passed. A six-row native proof passed.
    Integrated six-row SHA plan/native execution and normal C++ verification
    passed: 1654.925s MPC proof, about 1.12GB peak RSS per party. Native fixture
    is output-only. The later six-row Poseidon/native path binds an actual
    four-input/FROST synthetic transaction. Actual funded DKG settlement and
    public-only verification also pass; SHA remains an older measured reference.
- [x] Compare BN254 Poseidon2 emulation in Pallas against the demonstrated SHA bridge.
  - Human supplied a performance review on Oct 4. Keep the working SHA path as
    a baseline; measure fully constrained non-native arithmetic before adopting
    it. Do not claim a 10s–1m bound without an actual proof. Avoid repeated CPU
    builds: the ARM AES configuration issue has been resolved and measured.
    One native Poseidon row passed with codec-only configuration: 170.788s total,
    11,200 proof bytes, including amount/recipient/hash/cmx/index rejection.
    Hash-only Pallas proof stage: 39.126s; REP3 row proof: 2.262s. Next measure
    six native rows with one reused key and sequential proofs. Completed: native
    808.534s including key/proof/verification/adversarial checks; MPC 8.170s versus
    SHA 1654.925s. Both normal C++ proofs and all six native bindings passed.
    Initial native run had memory pressure; measurements are single-host.
- [x] Four-ticket durable funding ledger and one-note/one-ticket enforcement on controlled regtest.
  - Duplicate/wrong/late deposits, note reuse, reservation/cancellation conflicts,
    reorg pauses, locks, truncated tails and receipt retention pass selected tests.
    Actual RPC captures → SDK ciphertext/witness/nullifier scanning → client/pair
    proofs → durable signed admission → exact native reservation/confirmed receipts
    pass. Restart compares the whole actual re-derived scan, not cached balances.
    General advancing public-chain wallet/reorg handling remains separate work.
- [x] Deliver private refund amounts from MPC without opening maximum bids.
  - Legacy deterministic mask commitments reveal two-candidate refunds; that
    transcript privacy claim is withdrawn after a public-only attack reproduced
    all four candidates. Versioned delivery_private4 now adds independent secret
    commitment blinders; actual REP3 and normal C++ verification pass. Native
    consumption and offline exact cached DKG settlement recovery pass with v2.
    Operators learn refunds and outcome inference remains; full maximum bids are
    not opened. Final public settlement bundles exclude the delivery transcript.
- [x] Deliver separately masked winner flags, including zero-price ties.
  - Actual delivery_flags4 REP3 proof and normal C++ verification pass; independent
    private mask/blinder pairs and domains prevent the legacy bit-candidate oracle.
    Price/root/cipher/flag changes reject. The small-blinder audit is a regression,
    not an exhaustive hiding proof. Native zero-price remains a separate gate.
- [x] Explicit shared18 six-row native integration and public-only replay.
  - Blinded v2 delivery, six fresh bindings, actual MPC, PCZT/FROST/native proof
    and six decryptions pass with synthetic funding in 709.426s. Public replay
    passes in 103.945s. Original working33 default/fingerprint is preserved;
    known versioned metadata dispatches only compiled circuits and fixed keys.
- [x] Run a new funded v2 native auction through confirmation.
  - Fresh price-25 book, new local DKG escrow, four confirmed deposits, 12 signed
    receipts and three signed final-book endorsements pass. Six fresh shared18
    bindings, committed MPC, exact PCZT/DKG FROST/native transaction, full-node
    acceptance, two confirmations, all six exact receipts and three finalized
    journals pass. Native construction 819.180s; no full maximum-bid list/master
    escrow key read. Public certified replay passes in 100.871s with ten changed
    certificate/book/recipient/layout cases rejected. Session 24305 is complete.
- [x] Complete zero-price native delivery and full transaction execution (offline).
  - Consumer now requires independently verified v2 winner bits for reserve-zero
    books, freshly re-derives the opening from the transcript/pads and checks
    count/refund/conservation. Six native unit tests pass. Actual price-0/sold-3
    committed MPC, six fresh shared18 bindings, four threshold spends, native
    proof/signatures and all six decryptions pass in 742.670s. Synthetic funding,
    trusted dealer, no chain broadcast. Local proof-backed per-ticket status
    receipts are now demonstrated separately below; network delivery remains.
    Public replay passes in 103.898s, and seven transaction/action/layout changes
    reject, including substitution of the other valid compiled circuit/VK.
- [x] Local proof-authenticated per-ticket winner/refund receipts, including price zero.
  - Recipient supplies own expected ticket, reads two private pads only and
    verifies the fixed C++ flags proof first. Zero-price receipt transcript links
    the exact verified native settlement context/price/sold. Positive/no-sale
    cases also pass; 72 substitutions reject. This is a private proof opening,
    not a signature or replacement for admission/funding authentication.
- [ ] Private authenticated network delivery and client UI for ticket status receipts.
  - Six-output NU7 preparation now inserts canonical ticket memos into the four
    refund outputs before IO finalization. Positive/zero actual flags delivery,
    fixed C++ verification, native memo decryptions and old-stack V3 note checks
    pass in9.300s/7.937s;26 pre-sign changes reject. The full signing path checks
    receipt/settlement context/price/sold equality before FROST. The later full
    synthetic six-output memo transaction passes in1015.173s; all four own-key
    receipt openings and public-only replay reference the same transaction.
    Actual funded/admitted public-chain receipt delivery remains unfinished.
  - Actual encrypted memo transport component passes for four recipients at
    zero/positive price: native PCZT/proof/FROST, own-key decryption, fixed C++
    flags verification and exact ticket openings. 152 recipient substitutions
    and24 cross-recipient decryptions reject; encrypted memo mutation fails native
    authorization and AEAD. Fabricated one-input/four-output transport transfers,
    not the six-output auction settlement. Integrating memos into that settlement,
    an advancing chain, registered receiver identities and client UI remain.
  - A recipient command now freezes public proof inputs, verifies the compiled
    C++ flags key/proof before reading the receiver key, authenticates the native
    transaction and checks its decrypted memo against the expected book/ticket.
    Zero/positive four-recipient runs pass in18.227s/14.353s;12 fail-closed client
    attacks reject. The initial positive run's below4GiB resource guard refusal
    is preserved; a fresh checkpoint succeeds after headroom returns. The native
    transport is still separate from actual six-output auction settlement.
- [x] Resource guard abort stops nested MPC children without affecting parent/unrelated groups.
  - Synthetic old behavior reproduced a child surviving its parent's resource
    abort. Guard now execs into a fresh owned group with a marker matching its
    actual pgid; nested MPC runners inherit it and standalone runs keep their own
    groups. Actual nested-stop/owned-child tests and guarded flags/C++ proofs pass.
- [x] Reject changed complete cached binding proofs without regeneration/signatures.
  - The exact copied partial-Prepare negative gate changes one cached proof;
    trusted fixed-VK verification rejects before proving-key preparation or FROST.
    No journal changes, regeneration or original artifact changes occur.
- [x] Signed acceptance receipts and consistent final root for the four-ticket gate.
  - Three distinct application Ed25519 keys endorse identical authenticated
    instructions. Per-ticket fsynced locks reject equivocation after restart.
    All-three final certificates now check complete frozen local funding and
    locked receipts, persist a final-book signature lock, detect omitted receipts
    and match all accepted entries/root and six recipient commitments to actual
    MPC public inputs. Certified public-only native/MPC replay passes (125.697s),
    with seven changed authority/signature/intent/book/root/recipient cases rejected.
    Four actual-regtest deposits now pass client-proof/pair admission before the
    real wall-clock cutoff, durable signed receipts and post-cutoff certification.
    Private wire receipts reload and reverify all signatures. Pre-FROST enforcement
    passes before FROST in the actual funded DKG run; independent operator hosts
    and general live-chain recovery remain.
    These signatures attest admission, not a public funding proof.
- [x] Four-ticket MPC result → settlement commitment → PCZT → proof checks → native signing.
  - Actual four-input/FROST transaction passed all six native/MPC row checks,
    signing/extraction and recipient decryption. First run uses oracle fixture
    fills. Different-book price-20 integration now passed proved private refunds,
    actual six outputs, FROST/native proofs and all recipient decryptions (1167.489s).
    Public-only replay passed for both runs. Separate public bundle replay passes
    without private files and rejects transaction signature/trailing bytes/action
    metadata changes. Actual controlled-chain DKG funding and authenticated
    admission/receipt checks also pass; public NU7 funding remains separate.
- [x] Seller payment and refunds in one native transaction on actual isolated regtest.
- [x] Six recipient wallets confirm amounts; public verification checks transaction identity.
  - Actual isolated chain confirms all six payments and consumption of all four
    funding notes. Two controlled confirmations are checked through trusted RPC,
    separate from public-only proof/signature replay; no SPV/public finality claim.
- [x] Controlled-chain crash recovery of exact built/prepared/signed native settlements.
  - Partial broadcast/finality updates across the three journals are idempotent
    for the same reserved digest, rejecting substitutions and skipped stages.
    Native construction stores a private verified PCZT before reservation.
    Early unapproved PCZT now checkpoints exact output openings before proving;
    pre-reservation exact built-PCZT/partial-proof resume is implemented and
    passed against a deliberately interrupted two-proof actual DKG construction.
    Exact source PCZT/rows/commitments reused; two cached proofs reverified,
    four missing proofs generated; changed amount rejects before reservation.
    Exact same-purpose Prepared-stage gate now passes full native/MPC/FROST on
    copied historical prefixes (two Prepared, one Frozen); six existing proofs
    reverified, zero duplicate reservations, one new Prepare, no original changes
    or RPC/broadcast. Cancellation-purpose substitution rejects before native row
    proving/signing. This is offline, not another funded-chain run. Signed native recovery
    passes on the actual completed DKG auction: original checkpoint, exact bytes,
    native/application proofs, all six SDK receipts and four spent nullifiers
    reverified. Zero re-sends/blocks/journal writes; canonical height 114 retained.
    General live reorg/expiry remains a separate unfinished gate.
    Complete-cache offline recovery now uses only the original trusted VK, reuses
    all six verified proofs and skips the unused proving key. The v2 delivery/
    MPC/FROST gate passes with unchanged original journals and exact effects;
    runtime/disk measurements are recorded without a controlled speedup claim.
    New explicit shared18 exact-cache recovery also passes under the nested-group
    resource guard in 232.798s: six reused proofs, no PK, identical effects,
    zero duplicate Prepare writes and unchanged original source/journals. This
    reuses spent historical notes offline; no additional RPC/broadcast/funding.
  - The interrupted row-0 run had no reservation/signature. Existing deposits,
    cutoff, DKG packages and all three Frozen journals survived the PC reboot;
    rerunning with fresh output artifacts, one heavy prover and resource guards.
- [ ] Cancellation, late/unaccepted deposits and reorg-safe state transitions.
- [ ] Resolve fee/dust policy for wrong-amount deposits before wallet integration.

## Priority 2 · presentation and reproducibility

- [ ] Bidder, seller and verifier UI with accurate trust/receipt states.
  - First local receipt/verifier UI is implemented with actual fixed C++/native
    recipient verification, positive/zero demo identities and real corrupt-proof
    refusal. Four winner/loser API cases,15 HTTP/input rejections and two proof
    refusals before key access pass. No browser keys/pads or fake bid submission.
    Live admission, bidder authentication, seller and full settlement views remain.
    A public settlement verification action is also connected to the historical
    complete price-25 six-output regtest bundle and independently supplied
    public operator authority; no browser-supplied paths/keys are accepted.
    Actual public-only six-row/native/MPC/accepted-book API replay passes in
    103.153s and result rendering is confirmed; five boundary substitutions
    reject. Public snapshot hashes equal the original, with no private artifacts.
- [ ] Four-bid repeatable demo, including intentionally rejected tampering.
- [ ] Benchmark four bids, then attempt 32 without weakening the guarantees.
- [ ] Threat model, dependency/license inventory and independent reproduction.
  - Generated selected-target locked inventory: 430 packages, no missing license
    declarations, registry checksums/git pins, no local absolute paths. Principal
    external tool/license notes are in docs/dependencies.md; external full graphs
    and another-host reproduction remain.
- [ ] Submission requirements confirmed from official event materials.
- [ ] OSS release readiness, video and submission package; human controls publication/submission.

## Maintenance loop

After completing a queue item, inspect the resulting implementation for concrete
correctness, privacy, reliability and usability gaps. Add only useful scoped tasks,
record evidence, and continue. Do not repeatedly rerun passing checks without a
change or unresolved concern. Keep scope centered on the four-bid complete flow.
