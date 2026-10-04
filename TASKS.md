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
- [ ] P0.3 Validate shares against bid commitments; reject bad shares and duplicate tickets.
  - Client ZK proofs bind bid openings to all three share-pair hashes. All 12
    delivered pairs and the committed MPC root verified in the synthetic gate;
    changed shares, price and root were rejected. Signed acceptance/funding is next.
- [x] P0.4 Pin the Ironwood/PCZT/FROST stack; offline native transaction and signature tests.
  - Source pinned; upstream's 3 v6 sighash regression tests pass. Own transaction
    construction, native proofs, threshold signatures, extraction and 6 recipient
    decryptions passed. Both ordinary and zero seller/operator output cases passed.
- [ ] P0.5 Receive testnet funds, sign 2-of-3, broadcast and decrypt the recipient note.
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
    is output-only; actual four-input/FROST transaction binding remains.
- [ ] Compare BN254 Poseidon2 emulation in Pallas against the demonstrated SHA bridge.
  - Human supplied a performance review on Oct 4. Keep the working SHA path as
    a baseline; measure fully constrained non-native arithmetic before adopting
    it. Do not claim a 10s–1m bound without an actual proof. Avoid repeated CPU
    builds: the ARM AES configuration issue has been resolved and measured.
    One native Poseidon row passed with codec-only configuration: 170.788s total,
    11,200 proof bytes, including amount/recipient/hash/cmx/index rejection.
    Hash-only Pallas proof stage: 39.126s; REP3 row proof: 2.262s. Next measure
    six native rows with one reused key and sequential proofs.
- [ ] Funding ledger and one-note/one-ticket enforcement.
- [ ] Signed acceptance receipts and consistent final root.
- [ ] MPC result → settlement commitment → PCZT → proof checks → native signing.
- [ ] Seller payment and refunds in one native transaction.
- [ ] Recipient wallets confirm amounts; public verification checks transaction identity/finality.
- [ ] Cancellation, late/unaccepted deposits and reorg-safe state transitions.
- [ ] Resolve fee/dust policy for wrong-amount deposits before wallet integration.

## Priority 2 · presentation and reproducibility

- [ ] Bidder, seller and verifier UI with accurate trust/receipt states.
- [ ] Four-bid repeatable demo, including intentionally rejected tampering.
- [ ] Benchmark four bids, then attempt 32 without weakening the guarantees.
- [ ] Threat model, dependency/license inventory and independent reproduction.
- [ ] Submission requirements confirmed from official event materials.
- [ ] OSS release readiness, video and submission package; human controls publication/submission.

## Maintenance loop

After completing a queue item, inspect the resulting implementation for concrete
correctness, privacy, reliability and usability gaps. Add only useful scoped tasks,
record evidence, and continue. Do not repeatedly rerun passing checks without a
change or unresolved concern. Keep scope centered on the four-bid complete flow.
