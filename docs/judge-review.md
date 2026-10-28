# DarkCall — review guide

DarkCall proves a sealed-bid auction's clearing price without opening a single
bid, and proves that the shielded Zcash payouts match it. This guide says what
to check, in what order, and what each check proves.

## Five minutes, on the live demo

| Step | Where | What you see |
| --- | --- | --- |
| 1 | [Verify a real Zcash settlement](https://49-12-250-128.sslip.io/demo/settled) | The Oct 19 testnet auction from the video. As the public you see the price and the transaction, not the bids. As Bidder 01 you open only its own receipt: won, refund 79. |
| 2 | **Re-run verification** | The four claims checked again with no secrets, in about 20 s: the price, the six payouts, the Zcash transaction and the signed book. |
| 3 | **Try to cheat** | Change one accepted bid (2 s), claim a higher price, or swap two payout proofs. Each is refused by the check it targets; an unchanged control copy passes. |
| 4 | **Inspect the proof trail** | The public data behind each claim (commitments, the signed root, the proof's public inputs, each payout row and its note commitment) with 35 consistency checks. |

## With more time

- **The 2-minute video**, [darkcall-demo-v3.mp4](../demo/media/v3/darkcall-demo-v3.mp4):
  a live auction cleared on secret shares, then the testnet settlement checked
  and two cheats refused. Sped-up spans state the real time they cover.
- **Run a new auction**, [on the live demo](https://49-12-250-128.sslip.io/demo/run),
  about 25 minutes on public testnet: create it, seal four demo bids, fund,
  clear, open your result, settle, open your receipt. Done end to end on Oct 27
  in 24 min 16 s ([record](../evidence/2026-10-27-hosted-testnet-auction.json)).
- **The protocol, stage by stage**, [design/protocol.md](design/protocol.md): who
  sends what, who learns what, what is proven and what is trusted, from the sealed
  bid to the public check.
- **Verify on your own machine.** `make verify` with the pinned tools
  ([reproduce.md](run/reproduce.md)), about 20 s per bundle. Or
  `scripts/docker_verify.sh`, which rebuilds every pinned tool from committed
  source in a clean Linux container and verifies every bundle (Docker, 15 GB
  free, about 20 minutes the first time). Neither needs a key, bid or witness.

## What each proof establishes

| Check | Proven by | Fails when |
| --- | --- | --- |
| A bid commitment matches its three REP3 share pairs | Client Noir proof (`bid_input`), checked at admission | Shares or commitment changed |
| The accepted book is the four admitted tickets | Three operator signatures over the root; the root equals the MPC statement's root | A ticket is omitted, swapped or added |
| The clearing price, winners and refunds follow the rule | REP3 coNoir proofs (`delivery_flags4`, `settlement_poseidon4`) over committed bids | Price, plan or a row changes |
| Each transaction output pays the planned amount to the planned recipient | Six Halo2 output-binding proofs against native note commitments | Amount, recipient, cmx or action index changes |
| The transaction is authorized and valid | Native Zcash proof and 2-of-3 FROST signatures | Any byte of the effects changes |
| A bidder's own result is authentic | Fixed receipt proof, then the memo opened with the bidder's own viewing key | The receipt proof or the book is altered; refused before the key is read |

A verifier takes the operators' admission keys from a source it trusts, never
from the bundle itself.

## What is assumed

| Question | Answer |
| --- | --- |
| Can one operator see the bid list? | No, with at most one semi-honest corrupt operator (REP3). Two colluding operators can rebuild it. |
| Can operators see who won? | Yes. They learn winners and refunds, never the bid book. When only one bid loses, as in the demo, the winners tell them that bid was the price. |
| Who needs a viewing key? | Verifiers, none. Each bidder, its own, for its own receipt. The three operators, the escrow's, to admit deposits. None is published. |
| What does the public price reveal? | It is the highest losing bid or the reserve, so that one value is public by the rule. Who placed it is not. |
| Can operators drop a bid? | The public check cannot rule it out. Each bidder holds all three operators' signed acceptance and sees if its ticket is missing; outsiders cannot. |
| Who controls the funds? | Any two of three DKG FROST signers. No master key exists; bidders have no unilateral refund. |
| What if an operator stops? | The other two refund every deposit, signing 2-of-3 without the stopped operator's key share, and a refunded book cannot also settle. Shown on regtest only ([record](../evidence/2026-10-24-cancel-refund.json)); not offered on public testnet, including the live demo. If two stop, the deposits wait in escrow. |
| Is funding proven publicly? | No. Operator signatures attest it, and the bundle does not prove chain inclusion. |
| Who saw the demo bids? | Each bidder's client, which in this demo runs on the same host as the operators. Operators receive only shares and proofs. One test wallet funds all four tickets. |
| Can bidders use their own wallets? | The deposit format allows it: one Ironwood payment with a memo binding it to the ticket, which a ZIP-321 request can carry. No consumer wallet sends NU7 testnet transactions yet. |
| Testnet or mainnet? | Public testnet only, read through one trusted lightwalletd provider. |
| How large can an auction be? | Four bid slots and six outputs. Clearing was measured at 8 bids in 6.5 s; larger settlements are estimated, dominated by one Halo2 binding per payout ([record](../evidence/2026-10-21-bid-scaling.json)). |

## Records to start with

- [Oct 27 · live website on public testnet](../evidence/2026-10-27-hosted-testnet-auction.json): a new auction through the hosted demo.
- [Oct 19 · public testnet](../evidence/2026-10-19-public-testnet-settlement.json): the auction in the video.
- [Oct 21 · cheat lab](../evidence/2026-10-21-cheat-lab.json): each dishonest change refused by its own check.
- [Oct 18–19 · clean-room Linux](../evidence/2026-10-18-clean-room-linux-verification.json): every bundle verified from source with no secrets.
- [Attack and fix](design/operator-fill-delivery.md): an earlier refund delivery leaked to a two-candidate guess; we reproduced it and replaced it.

Every record: [evidence index](evidence-index.md). The full trust model and
measurements: [DarkCall in detail](details.md).
