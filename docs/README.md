# Documentation

Start with the [README](../README.md) for the problem, the idea and the trust
model, then the [review guide](judge-review.md) for what to check in what order.

## Understand it

| Document | What it covers |
| --- | --- |
| [judge-review.md](judge-review.md) | Review guide: the claims, where each is checked, and questions judges ask |
| [protocol.md](protocol.md) | Protocol specification (Japanese): goals, threat model, roles and design |
| [input-commitments.md](input-commitments.md) | The bidder's input proof: commitment, REP3 share pairs and their checks |
| [accepted-root.md](accepted-root.md) | Acceptance receipts and the signed accepted book |
| [operator-fill-delivery.md](operator-fill-delivery.md) | Private refund and winner delivery, the refund-guessing attack and its fix |
| [settlement-rows.md](settlement-rows.md) | The 224-byte payout row shared by the auction proof and the bindings |
| [output-binding.md](output-binding.md) | Binding a payout row to a native Zcash note commitment |
| [non-native-poseidon.md](non-native-poseidon.md) | BN254 Poseidon2 recomputed inside a Pallas circuit |
| [design-reviews.md](design-reviews.md) | Design reviews and the changes they led to |

## Run it

| Document | What it covers |
| --- | --- |
| [reproduce.md](reproduce.md) | Build the pinned tools and reproduce the public verification |
| [demo.md](demo.md) | Run the demo: the settled auction, a new auction, checks |
| [local-rehearsal.md](local-rehearsal.md) | The seller and bidder flow, with same-book settlement |
| [regtest.md](regtest.md) | The isolated NU7 regtest node and the funded settlement driver |
| [native-gate.md](native-gate.md) | Native transaction construction and the checks before FROST signing |
| [wallet-bridge.md](wallet-bridge.md) | Claims, receipts and operator journals |
| [hosting-plan.md](hosting-plan.md) | The hosted live demo: server, image and deployment |
| [toolchains.md](toolchains.md) | Cryptographic toolchain pins and how they are checked |

## Records

| Document | What it covers |
| --- | --- |
| [development.md](development.md) | What was built when during the hackathon, and what is not done |
| [evidence-index.md](evidence-index.md) | Every committed run record, with the ones to start with |
| [dependencies.md](dependencies.md) | Pinned dependencies, versions and licenses |
| [submission-draft.md](submission-draft.md) | The ZECATHON submission text |
