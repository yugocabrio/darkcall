# DarkCall — submission draft

Track: **Private Markets**. The author controls publication, the video and the
submission itself.

## One line

Sealed-bid uniform-price auctions on Zcash: one public price, bids that no
operator reconstructs, and one shielded settlement that anyone can verify.

## The problem

Selling a batch of identical units fairly, such as tickets, allocation slots,
compute or ad inventory, works best as a uniform-price sealed-bid auction. In
practice bidders must trust an auctioneer with their maximum willingness to pay,
or the bids are revealed after the close so that anyone can audit the result.
Either way, each bidder's maximum price leaks, and in on-chain settlement so
does who paid whom.

## What DarkCall does

1. The seller creates an auction: units, reserve, deposit budget, close time.
2. Each bidder commits to a maximum bid and splits it into REP3 secret shares
   for three operators. A client Noir ZK proof binds the commitment to the
   shares.
3. The bidder deposits budget + fee into a shielded 2-of-3 FROST escrow created
   by DKG. Operators check the deposit, the proof and the shares, sign
   acceptance receipts, and at the cutoff sign the final book (its root).
4. The three operators run the clearing as MPC (coNoir/REP3). They produce a ZK
   proof of the price, the winners and every refund without reconstructing any
   bid. Refunds and winner flags are delivered privately with blinded
   commitments.
5. One shielded NU7 transaction pays the seller, refunds all four bidders
   (budget − price for winners, full budget for losers) and pays the operator
   fee. Six Halo2 proofs bind each output's amount and recipient to the proven
   settlement rows. Two of three FROST signers authorize it only after all
   proofs and the signed book check out.
6. Each refund note carries an encrypted memo receipt. A bidder verifies the
   proofs first, then decrypts only their own outcome with their own viewing
   key. Anyone can verify the public bundle with no secrets at all.

## Why it is different

Auditable sealed-bid designs usually **disclose the bids to prove the result**.
DarkCall **proves the result without disclosing them**. Neither the public nor
any single operator learns who bid what, and no winner's maximum bid is ever
published. In this one-machine demo, the local bidder helper does see the typed
bid. Only the clearing price
and the units sold are public. By the auction rule, the price equals the
highest losing bid or the reserve. This needs all three parts. MPC keeps the bids hidden from any one
operator. Zero-knowledge proofs make the hidden computation checkable. Native
shielded settlement keeps the payments private while proofs tie them to the
result.

## What works (single-host prototype, isolated Zebra NU7 regtest)

- **The full flow in the browser, in protocol order.** Create an auction and
  seal four bids (client ZK proof + REP3 shares + signed pre-check). Certify the
  book, then **Fund on Zcash**: four real shielded deposits and a chain
  admission that re-verifies the same proofs and shares under a certificate
  over the same root. Then clear the book by MPC (about 9 s) and open your own
  result.
- **Settle on Zcash, same book.** A fresh MPC delivery inside settlement, six
  Halo2 output bindings, 2-of-3 DKG FROST signing, Zebra acceptance, two
  confirmations and six wallet scans. On Oct 18 browser-created auctions settled
  this way. The headline six-step run (Oct 19) has deposits `5e9432e5…`, price
  21 with 3 sold, and settlement `e3ffa436…84769754`. Its refunds were
  79/79/100/79 and the seller received 63.
- **A 2-minute video from one real auction.** `demo/media/v2/darkcall-demo-v2.mp4`
  is captured by a reproducible headless script with labeled time cuts.
- **Public verification without secrets.** About 2 minutes. It covers native
  authorization, six output bindings, the auction proof and the operator
  certificate. Corrupted proofs, transactions, books and authorities are
  rejected.
- **Private receipts.** Each bidder decrypts only their own memo. A corrupted
  receipt proof is rejected before the bidder's key is read.
- **An earlier recorded settlement** (`47019e02…a211938`) can be checked
  instantly in the same UI.

## Trust model, stated plainly

- Bid secrecy: at most one semi-honest corrupt operator (REP3). Two colluding
  operators can reconstruct bids. Not malicious-secure.
- Custody: 2-of-3 FROST from DKG. Two colluders can move funds. No unilateral
  refund.
- Operators learn winners and refunds and hold escrow viewing capability.
- The public clearing price equals the highest losing bid, or the reserve. That
  one value is public by the rules; who placed it is not. Small markets allow
  further inference.
- Funding is attested by operator signatures, not proven in zero knowledge. The
  public bundle does not prove chain inclusion.
- Demo scope: one host runs every party. The localhost helper sees typed demo
  bids. Deposits come from one local test wallet right after the close, before
  clearing (in a deployment each bidder's own wallet deposits). This is
  isolated regtest, not testnet or mainnet. There are four bid slots and default
  terms only.
- We found and fixed a refund-guessing leak in an earlier delivery commitment.
  The attack and the fix are documented in the repository.

## Built during the hackathon

Our original code is the circuits (`bid_input`, `delivery_flags4`,
`settlement_poseidon4`), the Halo2 output-binding circuit, the NU7 settlement
SDK (PCZT, DKG FROST, memos, scanner, verifier), the admission protocol
(claims, receipts, journals, signed book), the drivers and the browser UI. They
build on TACEO co-snarks (coNoir), Noir, Barretenberg, librustzcash/orchard,
frost-tools and Zebra at pinned versions. See `docs/dependencies.md`.

## Measured (shared 16 GiB M1)

Browser clearing: about 9 s. Funding + admission: about 2 min. Native settlement
of a fresh book: about 15 min (886–951 s, uninterrupted). Public verification:
about 2 min. Settle to verified, end to end: about 19–20 min. Own receipt: about
3 s.

## Links

- Video: `demo/media/v2/darkcall-demo-v2.mp4` (119 s, captions in `.srt`)
- Repository: `README.md`, then `docs/judge-review.md`
- Demo guide: `docs/demo.md` · Reproduction: `docs/reproduce.md`
- Evidence: `evidence/2026-10-18-fresh-book-native-settlement.json`
