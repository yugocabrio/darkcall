# DarkCall demo video v3: narration

Video length 119.7 s. Times are video times; read each line inside its window.

## 00:00–00:08 · slide 01-cover

DarkCall is a sealed-bid auction on Zcash where no bid is ever opened, yet the price is proven and paid out in one shielded transaction.

## 00:08–00:19 · slide 02-problem

Normally, checking the price means someone reads the bids: everyone after the close, or whoever makes the proof. And a proven price doesn't prove the payout.

## 00:19–00:23 · A seller lists three units

Let's run one, live. A seller lists three units.

## 00:23–00:36 · Each bid becomes three secret shares and a zero-knowledge proof

Each bidder's client splits its bid into three secret shares, one per operator, and proves in zero knowledge that they match its commitment. Operators check the proof and sign a receipt, but none can read a bid.

## 00:39–00:52 · REP3 clears on the shares · coNoir proves the price

At the deadline, the operators certify the book. All three clear it on the shares with REP3, and coNoir turns that into one proof of the price, in about eight seconds. Nobody rebuilds a bid, not even the prover.

## 00:52–00:58 · Each bidder opens only its own result

Twenty-one per unit, three sold. Each bidder opens only its own result.

## 00:58–01:13 · 2-of-3 FROST escrow · six Halo2 payout proofs · one shielded transaction

That run had no deposits. On October 19th the same four bids ran on public Zcash testnet: shielded deposits into a two-of-three FROST escrow, six Halo2 proofs binding every payout to the result, and one shielded transaction.

## 01:13–01:22 · Each bidder decrypts only its own receipt

Here is that settlement. Bidder one decrypts its own receipt: one unit, at twenty-one. Everyone else sees one shielded transaction.

## 01:22–01:26 · Each bidder decrypts only its own receipt

Bidder four: no unit, full refund.

## 01:26–01:36 · Public verification · no viewing key, no bid

Anyone can re-verify it with no secrets: the price proof, six payout proofs, the signed book and the Zcash signatures.

## 01:36–01:42 · Try to cheat: change one accepted bid

Now try to cheat. Change one accepted bid: the signed book refuses it.

## 01:42–01:45 · Try to cheat: claim a higher price

Claim a higher price:

## 01:45–01:50 · Try to cheat: claim a higher price

the price proof refuses it.

## 01:50–01:59 · slide 12-close

Private inputs, public checks. Every run is recorded in the repository, and make verify checks it on your own machine.

## Saying the names

REP3: "rep three" (レップ・スリー) · coNoir: "co-nwahr" (コー・ノワール) · Halo2: "halo two" (ヘイロー・ツー) ·
FROST (フロスト) · zatoshi (ザトーシ) · Zcash (ジーキャッシュ)
