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

- [ ] P0.1 Pin coNoir/Nargo/Barretenberg; compile a four-bid circuit.
- [ ] P0.2 Enter actual REP3 shares, jointly generate a ZK proof, verify using the normal verifier.
- [ ] P0.3 Validate shares against bid commitments; reject bad shares and duplicate tickets.
- [ ] P0.4 Pin the Ironwood/PCZT/FROST stack; offline native transaction and signature tests.
- [ ] P0.5 Receive testnet funds, sign 2-of-3, broadcast and decrypt the recipient note.
- [ ] P0.6 Prove one private settlement row binds to a native note commitment.
- [ ] P0.7 Reject amount, recipient and transaction substitution; measure runtime and memory.
- [ ] P0.8 Confirm planned padding, action count, bytes and fee in the native builder.

## Priority 1 · integration

- [ ] Canonical auction conditions, ticket IDs, accepted roots and settlement rows.
- [ ] Auction oracle and circuit agree on ties, insufficient demand and empty auctions.
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

