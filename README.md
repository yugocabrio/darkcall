# DarkCall

*Private, verifiable sealed-bid auctions on Zcash.*

**Sealed-bid auctions on Zcash where no bid is ever opened, not at the close and
not for any single operator. Even the zero-knowledge proof of the clearing price
is generated without opening the bids, and one shielded transaction that anyone
can verify pays the seller and refunds every bidder exactly as proven.**

- **Multi-party computation** (REP3). Each bid becomes secret shares; no single
  operator can read one.
- **Multi-party ZK proving** (coNoir). The operators compute the clearing price
  and jointly generate one zero-knowledge proof from those shares; no single
  prover ever sees the bid book.
- **Proof-bound shielded settlement** (Halo2). Four refunds, the seller and the
  operator fee: each of the six payouts has its own proof that its amount and
  recipient match the proven result, all in one shielded Zcash transaction.

ZECATHON 2026 · Private Markets track. **[Watch the 2-minute demo](demo/media/v2/darkcall-demo-v2.mp4)**:
one real auction, from creation to shielded receipts, with every time cut labeled.
**Settled on public Zcash testnet** from the browser on Oct 19: settlement tx
[`be1157c8…a1c8ad67`](https://testnet.zecblock.com/tx/be1157c884ce015e87993f105ebf44c3ab7f78e895d783fd002948c3a1c8ad67) at block 4,518,315 ([record](#settled-on-public-zcash-testnet)).

![DarkCall landing page: no bid is ever opened, the price is still proven, settled on Zcash; split, prove and pay as multi-party computation, multi-party ZK proving and proof-bound settlement; beside it, a real auction settled in this UI on public Zcash testnet](demo/media/landing-hero.jpg)

> **Demo scope, up front.** This prototype runs every role on one machine. Each
> bidder's own client builds the bid's proof and shares and is the only party
> that sees the typed bid; here it runs as a local service, and the three
> operators share the same host. The protocol keeps bids from
> the operators and the public; this demo shows that machinery working, not
> isolation between separate parties. Details are under
> [Trust model and limits](#trust-model-and-limits).

A seller offers identical units. Bidders commit secret maximum bids that are
split into secret shares for three operators. The operators compute one uniform
clearing price with multi-party computation (MPC) and generate its zero-knowledge
proof jointly from the same shares (multi-party ZK proving with coNoir), so the
bid list is never reconstructed, not even by a prover. One shielded NU7
transaction pays the seller, refunds every bidder and carries each bidder's
encrypted receipt. Six Halo2 proofs bind every output of that transaction to the
proven auction result. A public verifier checks all of it without a single
secret. Each bidder decrypts only their own outcome.

```
 browser / bidder                 three operators                         Zcash (NU7 testnet)
 ────────────────                 ───────────────                         ───────────────────
 max bid ─► commitment + REP3 ──► check client ZK proof + shares
            shares + ZK proof     sign acceptance receipts
 deposit budget+fee ────────────────────────────────────────────────────► 2-of-3 FROST (DKG) escrow
                                  cutoff → sign final book (root)
                                  REP3 MPC: price, winners, refunds  ─┐
                                  multi-party ZK proof (coNoir)       │
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
DarkCall adds what on-chain auctions lack. Outsiders cannot see what you bid or
whether you won. By design, no single operator holds your bid. Payment is
shielded ZEC, and the result is still verifiable.

**Why the settlement time is acceptable.** These are batch sales that close
once: an allocation round, a weekly ad slot, a block of seats. Bidding takes
minutes to days; settling in 15–20 minutes after the close, with every output
proven, fits that rhythm. Clearing itself, the moment the price is known, takes
about 9 seconds.

**What running one takes.** A seller and three independent operators (for
example the seller, a community member and an auditor), each with a laptop-class
machine. In a deployment each bidder runs the DarkCall client and deposits from
their own Zcash wallet, with the signed claim in the memo; in this demo one local
test wallet funds all four tickets. The operators never see a bid, and anyone
can check the outcome from the public bundle. Today this runs on public testnet;
mainnet follows NU7 activation on mainnet. Four bid slots per auction is the
size this prototype's circuits are compiled for.

## What is different

To let everyone check the price, other designs open the bid amounts at the close.
DarkCall proves the price without opening them, then pays out in shielded ZEC.

| | **DarkCall** | Open on-chain auction | Commit–reveal or published viewing key | Trusted auctioneer or key committee |
| --- | --- | --- | --- | --- |
| Bids hidden from the public | **Yes, never opened** | No | Only until the close | Yes |
| Bids hidden from whoever runs it | **Yes, from any single operator** | No | No | No, it decrypts them |
| Anyone can check the price | **Yes, by zero-knowledge proof** | Yes | Yes, by opening every bid amount | No, you trust it |
| Payments and refunds stay private | **Yes, one shielded transaction** | No | Possible, if paid shielded | Possible, if it pays shielded |
| Payout provably follows the result | **Yes, six output proofs** | Yes, by contract | By contract, or audited afterwards | No, the signers decide |

Common designs compared, not specific projects. "Any single operator" is the
REP3 bound: two colluding operators could reconstruct bids (see
[Trust model and limits](#trust-model-and-limits)).

Typical sealed-bid designs either trust an auctioneer with every bid or **reveal
the bids after the close** so that anyone can audit the result (commit-reveal, or
publishing a viewing key). DarkCall checks the result **without opening the
bids**. Neither the public nor any single operator learns who bid what, and the
winners' maximum bids are never revealed:

| Public | Hidden from the public (operators learn winners and refunds) |
| --- | --- |
| Auction terms, accepted-book root, clearing price, units sold | Who bid what; every winner's maximum bid |
| A shielded transaction and the proofs that bind its six outputs to the result | Who won and who lost; each refund; recipient addresses |
| Operator signatures over the accepted book | Bidder ↔ deposit linkage (shielded) |

The clearing price is, by the auction rule, the highest losing bid (or the
reserve). That one number is public; who placed it is not.

Correctness comes from proofs, not from disclosure: a multi-party ZK proof
(coNoir over REP3) of the clearing over the committed bids, a Halo2 proof per
transaction output, native Zcash authorization and an operator-signed accepted
book.

### Why hiding the amounts matters, not only the bidders

Hiding who bid is not enough when the amounts are published after the close:

- **A bid is a willingness to pay.** Publishing every amount gives the seller,
  and every rival, the full demand curve for the next sale.
- **Repeated sales leak strategy.** Bidders in a series of drops or allocation
  rounds can be profiled and outbid by their past amounts.
- **Amounts identify.** A distinctive amount, together with its timing, can link a
  pseudonymous bid back to a wallet or a person.

DarkCall publishes one number, the clearing price, and proves that it follows
from bids that are never opened.

## Settled on public Zcash testnet

On October 19, 2026 an auction was **created, funded, cleared and settled by
clicking through the browser UI on standard public Zcash testnet (NU7)**, with
testnet coins and the same proofs, admission and signing as below. It is the
auction in the video and under "See a settled auction":

| Step | Public testnet record |
| --- | --- |
| Fund | Deposit tx [`af6cf7cf…eff6354`](https://testnet.zecblock.com/tx/af6cf7cf6314bd946d76aec951f8277f12a4e27a97a335b81dbcbc241eff6354), block 4,518,254, 2 confirmations. 4 client proofs and 12 share pairs re-verified; 3 operators signed the same root |
| Clear | REP3 price **21**, 3 of 3 sold, 8.1 s, after funding |
| Settle | Settlement tx [`be1157c8…a1c8ad67`](https://testnet.zecblock.com/tx/be1157c884ce015e87993f105ebf44c3ab7f78e895d783fd002948c3a1c8ad67), block 4,518,315, 2 confirmations. Six output bindings, 2-of-3 DKG FROST |
| Public check | 102 s with no secrets; a corrupted receipt proof is refused by the fixed verifier |
| Own receipts | Ticket 01 won, refund 79; Ticket 04 lost, refund 100; each decrypted from its own memo |

Transaction links open ZecBlock, a third-party testnet explorer. The Zcash network
validated these native transactions; DarkCall's public verifier checks the auction
proof and the six output bindings:

```sh
./verify.sh --funded demo/artifacts/testnet-auction-nu7-v2 \
  --authority demo/authority/testnet-auction-nu7-v2.json
```

An earlier testnet auction driven through the same API by the HTTP harness
(settlement `8e6da99b…034f0db7`, bundle `testnet-auction-nu7-v1`) also passed
every refusal check: tamper, foreign capability, early settle and concurrent
proofs. Chain data came from one lightwalletd provider (`testnet.zec.rocks`)
over validated TLS; the SDK re-checks the chain, genesis and NU7 branch on
every connection, and the scanner checks every block's tree size and the final
root. Full record: [testnet evidence](evidence/2026-10-19-public-testnet-settlement.json).
`./demo.sh --chain testnet` runs Fund and Settle on testnet from the browser.

## It runs, end to end, for an auction you create

On October 19, 2026 an auction was **created, bid on, funded, cleared and
settled by clicking through the browser UI**, in protocol order, on an isolated
Zebra 7.0.0-rc.0 NU7 regtest. The same sealed tickets, commitments, shares and
accepted root `2d619728…558d12f9` carried through every step:

| Step | Result |
| --- | --- |
| Create + 4 sealed demo bids (77, 61, 43, 21) | 4 client ZK input proofs, 12 REP3 share-pair checks, 3 operator signatures per bid (pre-check) |
| Fund on Zcash | 4 real shielded deposits, tx `5e9432e5…d72f14be`, 2 confirmations. Same 4 proofs + 12 pairs re-verified against the deposits; operator certificate over the **same root** (121 s) |
| Clear | REP3 price **21**, 3 sold, coNoir proof verified in 7.8 s, after funding |
| Settle | Fresh REP3 delivery on the same shares, 6 Halo2 output bindings, 2-of-3 DKG FROST; Zebra accepted tx `e3ffa436…84769754` with 2 confirmations (driver 912 s) |
| Payout | 6 wallets scanned their exact notes: refunds 79/79/100/79, seller 63, operator fee 30000 |
| Public check | Public-only verifier passed in 105 s: native authorization, 6 bindings, auction proof, certificate |
| Own receipts | Ticket 01 WON / 79 and Ticket 04 LOST / 100, each decrypted from its own memo with its own key; a corrupted receipt proof was rejected before key use |

The scripted run of the same API (`scripts/check_fresh_settlement.py`) repeats
the flow. It also checks that clearing or settling before funding, a tampered
receipt proof, a foreign ticket capability and a concurrent proof are all
refused. Seven committed public bundles can be verified without secrets: the
two public-testnet auctions (one is the auction in the video), this six-step
regtest auction, the earlier regtest video run, two browser-created books from
Oct 18, and the Oct 5 recorded settlement `47019e02…a211938` with fixture bids. Exact records:
[browser settlements](evidence/2026-10-18-fresh-book-native-settlement.json),
[recorded settlement](evidence/2026-10-05-canonical-funded-nu7-memo-auction.json).

<table>
<tr><td><img src="demo/media/v2/04-sealed.jpg" alt="Four sealed tickets with the six-step bar"></td>
<td><img src="demo/media/v2/07-funded.jpg" alt="Deposits locked on Zcash regtest and admitted over the same root, before clearing"></td></tr>
<tr><td>Four sealed tickets: each a client ZK proof plus three operator signatures; no value shown.</td>
<td>Fund on Zcash: real shielded deposits, admitted over the same root, before any clearing.</td></tr>
<tr><td><img src="demo/media/v2/16-settled.jpg" alt="Settlement stages done, who learned what, and the settlement transaction"></td>
<td><img src="demo/media/v2/20-receipt-tamper.jpg" alt="A losing ticket's shielded refund decrypted, and a corrupted receipt proof rejected"></td></tr>
<tr><td>Settle: MPC delivery, six output bindings, FROST signing, confirmations, public check.</td>
<td>Each ticket opens only its own shielded receipt; a corrupted proof is rejected before the key is read.</td></tr>
</table>

Frames from the video run on public testnet (auction `4dde6fcb…`, settlement `be1157c8…a1c8ad67`); see
[demo/media/v2/manifest.json](demo/media/v2/manifest.json).

## Try it

**Verify every committed settlement in a clean Linux container, with one
command.** It needs Docker and at least 15 GB of free disk; the build takes
about 20 minutes the first time:

```sh
scripts/docker_verify.sh
```

It builds every pinned tool from source using only committed files, verifies
every committed bundle, and checks that a corrupted proof is rejected. On Oct 19
a fresh image built from commit `a7f67bd` verified all seven committed bundles,
including both public-testnet runs, with the strict tamper refusal (about 100 s
each; [record](evidence/2026-10-18-clean-room-linux-verification.json)). Remove
the image afterwards with `docker image rm`.

**Or verify on the host.** Build the pinned tools
([reproduce.md](docs/reproduce.md)), then run:

```sh
./verify.sh --funded demo/artifacts/browser-auction-nu7-v2 \
  --authority demo/authority/browser-auction-nu7-v2.json
```

**Run the whole product locally.** You need the pinned tools, Docker, and the
local NU7 regtest node and test bank ([regtest.md](docs/regtest.md)).

```sh
python3 scripts/regtest_nu7_node.py start
./demo.sh            # http://127.0.0.1:8877
```

Then: **Run an auction → Create auction** (default terms) → seal four demo bids →
wait for the close → **Certify closed book → Fund on Zcash** (real deposits and
chain admission, about 2 minutes) **→ Clear this auction → Open my result →
Settle on Zcash** (about 17 minutes on a 16 GiB M1, with live progress). After it finishes, **Open my shielded receipt** decrypts your
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
  winners.
- **The price is one bid.** Under these rules the public clearing price equals
  the highest losing bid, or the reserve. That single value is public by design;
  who placed it is not. Small books and repeated auctions allow further
  inference.
- **The bidder's client runs as a local service.** Only the bidder's own client sees the
  typed bid; operators receive shares and a proof. In this demo that client is a
  localhost service on the same machine as the operators. Deposits come from one local test wallet right
  after the close; in a deployment each bidder's own wallet deposits. All
  three operators run on one machine.
- **Funding is attested, not proven.** Admission signatures attest funding; they are not a public ZK funding
  proof. The public bundle proves transaction validity and output binding, not
  chain inclusion. Confirmations are observed through one lightwalletd provider
  on testnet, or on a controlled regtest node.
- **Testnet, never mainnet.** The video run and an API-driven run settled on
  public Zcash testnet with testnet coins, observed through one trusted
  lightwalletd provider; earlier browser runs used an isolated regtest. No
  mainnet funds are touched. The v6 settlement needs NU7, which is active on
  testnet and planned for mainnet on Nov 5, 2026.
- **Fixed size.** The committed circuits have four bid slots and six outputs. The native
  settlement path supports the default terms (3 units, budget 100, reserve 0 or 20).
  Four is the circuit size, not a speed limit: see [beyond four bids](#beyond-four-bids).
- **Demo-sized amounts.** Bids stay within a 100-zatoshi budget, and the fixed
  15,000-zatoshi processing fee per ticket pays the settlement's 30,000-zatoshi
  network fee and a 30,000-zatoshi operator fee output. The numbers exercise
  the cryptography end to end; they are not a pricing proposal.

An earlier delivery design leaked refunds to a two-candidate guess. We reproduced
the attack, withdrew the claim and replaced it with blinded v2 commitments:
[attack and fix](docs/operator-fill-delivery.md).

## Measured on a shared 16 GiB Apple M1

| Scope | Time |
| --- | --- |
| Client bid: input ZK proof + shares + 3 operator signatures | 0.5–0.6 s |
| Fund on Zcash: deposits + chain admission + certificate (includes a 120 s admission window) | 121 s |
| Browser REP3 clearing incl. coNoir proof and verification | 7.6–9.4 s |
| Native settlement of a fresh book, uninterrupted (2 threads) | 885–951 s |
| Settle → publicly verified (fund-first flow) | 998–1024 s |
| Public-only verification of a settlement (host or clean container) | 99–112 s |
| Own receipt opening (proofs checked before key use) | ~3 s |
| Recorded Oct 5 settlement (1 thread, memory pressure) | 1848 s |

These are single-host measurements, not a controlled benchmark.

### Beyond four bids

`scripts/measure_bid_scaling.py` derives N-bid versions of the three coNoir
circuits (only the bid count changes; the four-bid versions match the committed
circuits gate for gate) and runs the clearing proof with three REP3 parties on
synthetic bids. Record: [evidence/2026-10-21-bid-scaling.json](evidence/2026-10-21-bid-scaling.json).

| Bids | Clearing gates | Winner-flag gates | Settlement-plan gates | Joint clearing proof (3 parties) | Halo2 payout bindings, one after another |
| --- | --- | --- | --- | --- | --- |
| 4 | 10,247 | 11,894 | 24,016 | 3.1 s, ~0.3 GiB per party (measured) | 6 rows, ~11 min (measured in the Oct 18 run) |
| 8 | 26,953 | 30,020 | 48,901 | 6.5 s, ~0.6 GiB per party (measured) | 10 rows, ~17 min (estimated) |
| 16 | 90,847 | 96,562 | 128,984 | ~30 s, ~2.5 GiB per party (estimated, not run) | 18 rows, ~29 min (estimated) |

Gate counts are measured for all three circuits at every size. Only the
clearing circuit's joint proof was run (4 and 8 bids, both verified); the
winner-flag and settlement-plan joint proofs and a full settlement were not
run above four bids. All-pairs ranking makes the joint proof grow
quadratically, but it stays in seconds. The larger cost is linear: one Halo2
binding per payout (N refunds, the seller, the fee), about 90 s each here after
a one-time key. The rows are independent, so they could run in parallel; that
speed-up is not demonstrated, since the prototype proves them one after
another. Beyond a few dozen bids, all-pairs ranking should give way to a
sorting network.

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
| `docker/verify/` | Clean-room Linux build of the public verifier (`scripts/docker_verify.sh`) |
| `evidence/` | Measured, public-only run records |

Static evidence page for GitHub Pages: [docs/index.html](docs/index.html), built by
`scripts/build_evidence_site.py` and not yet published.

More: [judge review](docs/judge-review.md) · [submission](docs/submission-draft.md) ·
[demo guide](docs/demo.md) · [reproduction](docs/reproduce.md) ·
[protocol (Japanese design spec)](docs/protocol.md) · [status](STATUS.md) · [tasks](TASKS.md).

All code is MIT licensed; upstream components keep their own licenses
([dependencies](docs/dependencies.md)).
