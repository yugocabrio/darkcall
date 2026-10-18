# DarkCall

**Sealed-bid auctions on Zcash where the price is public, the bids never are, and
settlement is one shielded transaction anyone can verify.**

ZECATHON 2026 · Private Markets track.

A seller offers identical units. Bidders commit secret maximum bids that are
split into secret shares for three operators. The operators compute one uniform
clearing price with multi-party computation (MPC) and jointly prove it with a
zero-knowledge proof. Nobody reconstructs the bid list. One shielded NU7
transaction pays the seller, refunds every bidder and carries each bidder's
encrypted receipt. Six Halo2 proofs bind every output of that transaction to the
proven auction result. A public verifier checks all of it without a single
secret. Each bidder decrypts only their own outcome.

```
 browser / bidder                 three operators                         Zcash (NU7 regtest)
 ────────────────                 ───────────────                         ───────────────────
 max bid ─► commitment + REP3 ──► check client ZK proof + shares
            shares + ZK proof     sign acceptance receipts
 deposit budget+fee ────────────────────────────────────────────────────► 2-of-3 FROST (DKG) escrow
                                  cutoff → sign final book (root)
                                  REP3 MPC: price, winners, refunds  ─┐
                                  coNoir proof (no bid reconstructed) │
                                  6 Halo2 output-binding proofs ──────┤
                                  2-of-3 FROST signatures ────────────┴─► one shielded tx:
                                                                          seller · 4 refunds+memos · fee
 open own memo with own key ◄───────────────────────────────────────────── 
 anyone: verify public bundle (proofs + tx + certificate), no secrets
```

## Who it is for

Anyone who sells a batch of identical units and wants one fair price: event
seats, allocation slots in a token or NFT drop, sponsorship or ad slots, or
reserved compute. A uniform-price sealed-bid auction is the fair format: every
winner pays the same price, set by the highest losing bid (or the reserve), so a
bidder who wants one unit does best by bidding their true value.
DarkCall adds what on-chain auctions lack. Nobody learns your maximum price or
whether you won, payment is shielded ZEC, and the result is still verifiable.

## What is different

Typical sealed-bid designs either trust an auctioneer with every bid or **reveal
the bids after the close** so that anyone can audit the result (commit-reveal, or
publishing a viewing key). DarkCall checks the result **without revealing any
maximum bid, including the winners' bids**:

| Public | Private |
| --- | --- |
| Auction terms, accepted-book root, clearing price, units sold | Every maximum bid |
| A shielded transaction and the proofs that bind its six outputs to the result | Who won and who lost; each refund; recipient addresses |
| Operator signatures over the accepted book | Bidder ↔ deposit linkage (shielded) |

Correctness comes from proofs, not from disclosure: a coNoir (REP3) proof of the
clearing over the committed bids, a Halo2 proof per transaction output, native
Zcash authorization and an operator-signed accepted book.

## It runs, end to end, for an auction you create

On October 18, 2026 an auction was **created, bid on, cleared and settled by
clicking through the browser UI**. It settled on an isolated Zebra 7.0.0-rc.0
NU7 regtest. The same sealed tickets, commitments, shares and accepted root
carried through every step:

| Step | Result |
| --- | --- |
| Create + 4 sealed demo bids (77, 61, 43, 21) | 4 client ZK input proofs, 12 REP3 share-pair checks, 12 signed acceptances |
| Close + clear in the browser | REP3 price **21**, 3 sold, coNoir proof verified in 8.6 s |
| Settle → fund & admit | 4 real shielded deposits, tx `d11efd9a…5fd04abd`, 2 confirmations. Same 4 proofs + 12 pairs re-verified; operator certificate over the **same root** `04728ac2…ccbc739a` (121 s) |
| Settle → MPC, bind, sign | Fresh REP3 run inside settlement, 6 Halo2 output bindings, 2-of-3 DKG FROST; Zebra accepted tx `3fee2c7c…b5c0b5d7` with 2 confirmations (settlement 886 s) |
| Payout | 6 wallets scanned their notes: refunds 79/79/100/79, seller 63, operator fee 30000 |
| Public check | Public-only verifier passed (native authorization, 6 bindings, auction proof, certificate) in 102 s; Settle → verified 1117 s |
| Own receipts | Each bidder's memo decrypts with only their own key: won/lost and refund equal to the browser result |

A scripted run of the same API (`scripts/check_fresh_settlement.py`) repeats
this. It also checks that a tampered receipt proof, a foreign ticket capability,
an early settle and a concurrent proof are all refused. Three committed public
bundles can be verified without secrets: this browser auction, a CLI-driven
fresh book, and the earlier recorded settlement `47019e02…a211938` with fixture
bids. Exact records:
[Oct 18 settlements](evidence/2026-10-18-fresh-book-native-settlement.json),
[recorded settlement](evidence/2026-10-05-canonical-funded-nu7-memo-auction.json).

## Try it

**Verify a recorded settlement, with no secrets.** First build the pinned tools
([reproduce.md](docs/reproduce.md)), then run:

```sh
./verify.sh --funded demo/artifacts/browser-auction-nu7-v1 \
  --authority demo/authority/browser-auction-nu7-v1.json
```

**Run the whole product locally.** You need the pinned tools, Docker, and the
local NU7 regtest node and test bank ([regtest.md](docs/regtest.md)).

```sh
python3 scripts/regtest_nu7_node.py start
./demo.sh            # http://127.0.0.1:8877
```

Then: **Try DarkCall → Create auction** (default terms) → seal four demo bids →
wait for the close → **Certify closed book → Clear this auction → Open my result
→ Settle on Zcash**. Settlement shows live progress and takes about 20 minutes end
to end on a 16 GiB M1. After it finishes, **Open my shielded receipt** decrypts your
memo from the settlement transaction, and **Tamper with receipt** shows a
corrupted proof being refused before your key is read.
`./demo.sh --public-only` serves a key-less view that can verify and reject
tampering.

For a scripted run over the same HTTP API:

```sh
python3 -B scripts/check_fresh_settlement.py --output .local/fresh-settlement-check.json
```

## Trust model and limits

DarkCall is a working prototype, not production custody. These are its exact
assumptions:

- **Bid secrecy** holds against at most one semi-honest corrupt operator (REP3).
  Two colluding operators can reconstruct bids. It is not malicious-secure MPC.
- **Custody** is 2-of-3 FROST from a DKG. No escrow master key exists, but two
  colluding signers can move funds, and bidders have no unilateral refund path.
- **Operators see outcomes.** They hold escrow viewing capability and learn refunds and
  winners. The public price, small books and repeated auctions allow inference.
- **The demo helper sees typed bids.** In this local demo, the localhost helper that builds the client
  proof sees the typed demo bid. Deposits come from one local test wallet. All
  three operators run on one machine.
- **Funding is attested, not proven.** Admission signatures attest funding; they are not a public ZK funding
  proof. The public bundle proves transaction validity and output binding, not
  chain inclusion. Confirmations are observed on a controlled regtest node.
- **Regtest only.** It runs on an isolated regtest: no public testnet or mainnet funds.
- **Fixed size.** The circuits have four bid slots and six outputs. The native settlement path
  supports the default terms (3 units, budget 100, reserve 0 or 20).

An earlier delivery design leaked refunds to a two-candidate guess. We reproduced
the attack, withdrew the claim and replaced it with blinded v2 commitments:
[attack and fix](docs/operator-fill-delivery.md).

## Measured on a shared 16 GiB Apple M1

| Scope | Time |
| --- | --- |
| Client bid: input ZK proof + shares + 3 signed acceptances | 0.5–0.6 s |
| Browser REP3 clearing incl. proof and verification | ~9 s |
| Deposits + chain admission + certificate (includes a 120 s admission window) | ~2 min |
| Native settlement of a fresh book, uninterrupted (2 threads) | 886–951 s |
| End to end from Settle to publicly verified | 1117–1198 s |
| Public-only verification of a settlement | 103–112 s |
| Own receipt opening (proofs checked before key use) | ~3 s |
| Recorded Oct 5 settlement (1 thread, memory pressure) | 1848 s |

These are single-host measurements, not a controlled benchmark.

## Repository

| Path | Contents |
| --- | --- |
| `circuits/` | Noir circuits: `bid_input` (client), `delivery_flags4` (clearing and private delivery), `settlement_poseidon4` (six settlement rows) |
| `crates/output-binding` | Halo2 proofs that bind settlement rows to native note commitments |
| `crates/native-gate` | Funded NU7 construction, MPC/Halo2 checks before FROST signing |
| `crates/wallet-protocol`, `crates/wallet-bridge` | Claims, acceptance receipts, journals, signed final book |
| `tools/nu7-compat` | NU7 SDK: deposits, scanning, PCZT, DKG FROST, receipt memos, public verifier |
| `scripts/` | Drivers: `serve_demo.py`, `rehearsal.py`, `settle_fresh_book.py`, regtest and checks |
| `demo/` | Browser UI, public bundles and separately configured authority |
| `evidence/` | Measured, public-only run records |

More: [judge review](docs/judge-review.md) · [submission](docs/submission-draft.md) ·
[demo guide](docs/demo.md) · [reproduction](docs/reproduce.md) ·
[protocol (Japanese design spec)](docs/protocol.md) · [status](STATUS.md) · [tasks](TASKS.md).

All code is MIT licensed; upstream components keep their own licenses
([dependencies](docs/dependencies.md)).
