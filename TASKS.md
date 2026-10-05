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
- [ ] P0.7 Reject amount, recipient and transaction substitution; measure runtime and memory.
- [ ] P0.8 Confirm planned padding, action count, bytes and fee in the native builder.
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
    pass without shared Rust types; actual new-SDK output-binding proof is next.

- [ ] Canonical auction conditions, ticket IDs, accepted roots and settlement rows.
- [x] Initial oracle and circuit fixtures agree on ties, insufficient demand and empty auctions.
- [ ] Cross-language generated fixtures and committed-input circuit equivalence.
- [ ] Shared auction/native row commitment encoding with fully constrained hash links.
  - Canonical 224-byte SHA-256 format, linked Halo2 SHA and actual native row proof
    passed. Six-row Noir plan circuit compiles/tests pass. Complete MPC hash proof
    passed on the same private row using ARM AES-enabled coNoir (574.499s proof
    stage). Normal C++ verification passed. A six-row native proof passed.
    Integrated six-row SHA plan/native execution and normal C++ verification
    passed: 1654.925s MPC proof, about 1.12GB peak RSS per party. Native fixture
    is output-only. The later six-row Poseidon/native path binds an actual
    four-input/FROST synthetic transaction; real funding still remains.
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
- [ ] Funding ledger and one-note/one-ticket enforcement.
  - Private in-memory ledger tests pass: duplicate funding, wrong/late/unaccepted
    deposits, reorg pauses, settlement/cancellation reservation and empty auction.
    Durable fsynced private journal now passes restart, concurrent lock, truncated
    tail, symlink, reorg pause and refund retention checks. Persistent wallet
    scanning/finality remain. Native compact scanner and encrypted funding memo
    claim authentication now pass synthetic ciphertext tests. They are not yet
    connected to live RPC, persistent scanner replay or client-proof admission.
- [x] Deliver private refund amounts from MPC without opening maximum bids.
  - Fresh private field masks, mask commitments and normal C++ verification
    passed. Operators learn individual refunds; values stay out of public proofs.
- [ ] Deliver winner flags for zero-price auctions before supporting that full flow.
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
    is in the active funded run; independent operators and broader recovery remain.
    These signatures attest admission, not a public funding proof.
- [x] Four-ticket MPC result → settlement commitment → PCZT → proof checks → native signing.
  - Actual four-input/FROST transaction passed all six native/MPC row checks,
    signing/extraction and recipient decryption. First run uses oracle fixture
    fills. Different-book price-20 integration now passed proved private refunds,
    actual six outputs, FROST/native proofs and all recipient decryptions (1167.489s).
    Public-only replay passed for both runs. Separate public bundle replay passes
    without private files and rejects transaction signature/trailing bytes/action
    metadata changes. Funded chain integration and receipt authentication remain.
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
  - The interrupted row-0 run had no reservation/signature. Existing deposits,
    cutoff, DKG packages and all three Frozen journals survived the PC reboot;
    rerunning with fresh output artifacts, one heavy prover and resource guards.
- [ ] Cancellation, late/unaccepted deposits and reorg-safe state transitions.
- [ ] Resolve fee/dust policy for wrong-amount deposits before wallet integration.

## Priority 2 · presentation and reproducibility

- [ ] Bidder, seller and verifier UI with accurate trust/receipt states.
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
