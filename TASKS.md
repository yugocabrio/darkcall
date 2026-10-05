# DarkCall task queue

Latest human instruction: start development, commit and push regularly, continue
autonomously, incorporate later human/ChatGPT reviews.

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
- [ ] Migrate auction native stack to NU7 after isolated compatibility checks.
  - Standalone synthetic native gate and live tree reconstruction pass; root
    Cargo.lock, native binding dependencies and isolated NU6.3 chain are preserved.
    Seven new-to-old native V3 encoding/commitment/nullifier/psi/rcm_v3 byte cases
    pass without shared Rust types. New-SDK native output export and public cmx
    mapping checks pass; nine malformed public/private byte cases reject.
    Existing-circuit new-output proof hit the disk guard twice during keygen;
    no completed application proof. Preserve sources and avoid identical retries
    without capacity. Separate shared18 layout preserves the original configuration
    fingerprint and canonical/native constraints; actual new-output MockProver and
    five tamper cases pass. Actual shared18 SNARK
    and public-only new-native/row replay now pass (221.541s / 107.893s); one
    synthetic output only. Preserve working33 trusted verifier and original funded
    artifacts; full NU7 auction/wallet funding and live-chain freshness remain.
    New adapter verifies v2 delivery and prepares/checks all six unsigned NU7
    outputs at price25 and price0. Seven pre-sign malformed/substitution checks
    pass, including canonical PCZT trailing-byte refusal. Full fresh six-row
    proof, threshold native completion and public replay are implemented but
    unverified: 10 GiB start guard refused at 6.94 GiB free. Resume only after
    headroom changes; no guard relaxation, no false completed E2E claim.

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
    receipt/settlement context/price/sold equality before FROST; this new full
    six-row run is not yet demonstrated and still needs10GiB initial headroom.
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
