# DarkCall — submission draft

Track: **Private Markets**. The author controls publication, the video and the
submission itself.

## One line

DarkCall: private, verifiable sealed-bid auctions on Zcash. No bid is ever
opened: not at the close, not for any single operator, and not even to
generate the zero-knowledge proof of the price.

Longer: sealed-bid auctions on Zcash where no bid is ever opened, not at the
close and not for any single operator. Even the zero-knowledge proof of the
clearing price is generated without opening the bids, and one shielded
transaction that anyone can verify pays the seller and refunds every bidder
exactly as proven.

- **Multi-party computation** (REP3). Each bid becomes secret shares; no single
  operator can read one.
- **Multi-party ZK proving** (coNoir). The operators compute the clearing price
  and jointly generate one zero-knowledge proof from those shares; no single
  prover ever sees the bid book.
- **Proof-bound shielded settlement** (Halo2). Four refunds, the seller and the
  operator fee: each of the six payouts has its own proof that its amount and
  recipient match the proven result, all in one shielded Zcash transaction.

## What a reviewer can check, and how long it takes

1. **It settled on public Zcash testnet** (1 minute). Open the deposits
   [`af6cf7cf…eff6354`](https://testnet.zecblock.com/tx/af6cf7cf6314bd946d76aec951f8277f12a4e27a97a335b81dbcbc241eff6354)
   and the settlement [`be1157c8…a1c8ad67`](https://testnet.zecblock.com/tx/be1157c884ce015e87993f105ebf44c3ab7f78e895d783fd002948c3a1c8ad67)
   (blocks 4,518,254 and 4,518,315) on a testnet explorer.
2. **That auction, from creation to receipts** (2 minutes). The video is edited
   stills of that same run, with every time cut labelled.
3. **The settlement verifies with no secrets, and tampering is refused**
   (about 100 s per bundle once the pinned tools are built). `scripts/docker_verify.sh`
   builds every tool from committed source in a clean Linux container (about
   17 minutes of compiling on 4 cores) and checks all seven bundles: the auction
   proof, the six output bindings, the signed accepted book and a corrupted proof.
   The public bundle holds the terms, the price, the units sold, commitments,
   proofs and the shielded transaction; no bid, and no bidder's refund.

## Against the four judging criteria

- **Privacy.** No bid amount is ever opened, not at the close and not to any
  single operator. Only the clearing price is public; by the auction rule it is
  the highest losing bid or the reserve. Payouts and refunds are one shielded
  transaction, and each bidder decrypts only their own receipt. Limits, stated
  in full below: semi-honest REP3 (two colluding operators could reconstruct
  bids or move escrowed funds), and operators learn winners and refunds.
- **Usefulness.** Batch sales that close once, such as allocation rounds, ad
  slots or blocks of seats, where published bid amounts hand the seller and
  rivals the demand curve. Clearing takes about 9 s; settlement follows in
  15–20 min with every output proven. It needs three independent operators.
  The committed circuits take four bids; an 8-bid version of the clearing
  circuit proves jointly in 6.5 s (3.1 s at 4 bids, both measured). Settlement
  time is dominated by the per-payout Halo2 bindings, which grow linearly;
  larger settlements are estimated, not run.
- **Execution.** An auction was created, funded, cleared and settled by clicking
  through the browser on public Zcash testnet (settlement `be1157c8…a1c8ad67`,
  block 4,518,315), and that is the run in the video. Seven public bundles,
  both testnet runs included, verify in a clean Linux container with no network,
  and a tampered proof is refused.
- **Originality.** Multi-party ZK proving: the operators generate the price's
  zero-knowledge proof jointly from secret shares (coNoir over REP3), so no
  single prover ever sees the bid book. Proof-bound shielded settlement: six
  Halo2 proofs, one per payout (four refunds, the seller and the operator fee),
  bind each output of the native shielded transaction to the proven result.
  All code was written for this hackathon, starting Oct 4, 2026.

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
4. The three operators run the clearing as REP3 MPC and generate its ZK proof
   jointly from the same shares (multi-party ZK proving with coNoir): the price,
   the winners and every refund, without reconstructing any bid. Refunds and winner flags are delivered privately with blinded
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

To let everyone check the price, other designs open the bid amounts at the close.
DarkCall proves the price without opening them, then pays out in shielded ZEC.

| | **DarkCall** | Open on-chain auction | Commit–reveal or published viewing key | Trusted auctioneer or key committee |
| --- | --- | --- | --- | --- |
| Bids hidden from the public | **Yes, never opened** | No | Only until the close | Yes |
| Bids hidden from whoever runs it | **Yes, from any single operator** | No | No | No, it decrypts them |
| Anyone can check the price | **Yes, by zero-knowledge proof** | Yes | Yes, by opening every bid amount | No, you trust it |
| Payments and refunds stay private | **Yes, one shielded transaction** | No | Possible, if paid shielded | Possible, if it pays shielded |
| Payout provably follows the result | **Yes, six output proofs** | Yes, by contract | By contract, or audited afterwards | No, the signers decide |

Auditable sealed-bid designs usually **disclose the bids to prove the result**.
DarkCall **proves the result without disclosing them**. Only the clearing price
and the units sold are public; by the auction rule the price equals the highest
losing bid or the reserve. Neither the public nor any single operator learns
who bid what, and no winner's maximum bid is ever published.

Hiding who bid is not enough when the amounts are published after the close.
A bid is a willingness to pay: published amounts give the seller and rivals the
demand curve for the next sale, profile bidders across repeated drops, and can
link a distinctive amount back to a wallet. DarkCall publishes one number, the
clearing price, and proves it from bids that are never opened.

All three parts are needed:
- MPC keeps the bids away from any one operator.
- Zero-knowledge proofs make that hidden computation checkable.
- Native shielded settlement keeps the payments private, while proofs tie every
  output to the result.

Only the bidder's own client sees the typed bid; operators receive shares and a proof. In this one-machine demo that client is a local service on the same host.

## What works (single-host prototype)

- **Settled on public Zcash testnet, from the browser.** On Oct 19 an auction
  was created, funded, cleared and settled by clicking through the UI on
  standard testnet (NU7): deposits `af6cf7cf…eff6354` at block 4,518,254,
  settlement `be1157c8…a1c8ad67` at block 4,518,315, two confirmations each,
  public check in 102 s, own receipts won 79 and lost 100, and a corrupted
  receipt proof refused. This is the run in the video. An earlier API-driven
  testnet auction (`8e6da99b…034f0db7`) also passed every refusal check. Chain
  data came from one trusted lightwalletd provider.

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
- **A 2-minute video from one real testnet auction.** `demo/media/v2/darkcall-demo-v2.mp4`
  (auction `4dde6fcb…`, settlement `be1157c8…`) is captured by a reproducible
  headless script with labeled time cuts.
- **Public verification without secrets.** About 2 minutes. It covers native
  authorization, six output bindings, the auction proof and the operator
  certificate. Corrupted proofs, transactions, books and authorities are
  rejected.
- **Private receipts.** Each bidder decrypts only their own memo. A corrupted
  receipt proof is rejected before the bidder's key is read.
- **A recorded view of the same settlement.** `./demo.sh` serves the six-step
  auction under "See a settled auction". Open each bidder's receipt in about 3 s, or
  re-verify the whole settlement in about 2 minutes, without running a new
  auction. `./demo.sh --fixture-recorded` shows the Oct 5 fixture-bid run
  `47019e02…a211938`.

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
- Demo scope: one host runs every party. Each bidder's client, the only party
  that sees the typed bid, is a localhost service. Deposits come from one local test wallet right after the close, before
  clearing (in a deployment each bidder's own wallet deposits). It runs on
  public Zcash testnet (two auctions settled on Oct 19) and on an isolated
  regtest, never mainnet. There are four bid slots and default terms only.
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
of a fresh book: about 15 min (885–951 s, uninterrupted). Public verification:
about 2 min. Settle to verified: about 17 min (998–1024 s). Own receipt: about
3 s.

## Links

- Video: `demo/media/v2/darkcall-demo-v2.mp4` (119 s, captions in `.srt`)
- Repository: `README.md`, then `docs/judge-review.md`
- Demo guide: `docs/demo.md` · Reproduction: `docs/reproduce.md`
- Evidence: `evidence/2026-10-18-fresh-book-native-settlement.json`
