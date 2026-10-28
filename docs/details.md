# DarkCall in detail

The [README](../README.md) is the short version. This page keeps the full
explanation: who it is for, how it compares, the design choices, the recorded
runs, how to verify and run it, the trust model and the measurements.

## The flow

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

**A newsletter sells three sponsor slots every month.** Wallet makers,
exchanges and infrastructure firms bid; three independent operators run the
auction: the newsletter, a community foundation and an auditor. In an open or
commit–reveal auction every sponsor's ceiling is public after the close, so next
month's floor is set from it and rival sponsors learn each other's budgets. In a
negotiated deal the newsletter sees every offer and no one can check the price.
With DarkCall each sponsor bids its real ceiling, every winner pays the same
proven price in shielded ZEC, and neither the newsletter nor a rival sees a
sponsor's bid; only the clearing price, by the rules the highest losing bid, is
public.

The same holds wherever the same buyers come back or a bid is a valuation: a
treasury selling blocks of a holding to funds, for example.

A uniform-price sealed-bid auction is the fair format for such batch sales:
every winner pays the same price, set by the highest losing bid (or the
reserve), so a bidder who wants one unit does best by bidding their true value.
DarkCall adds what on-chain auctions lack. Outsiders cannot see what you bid or
whether you won. By design, no single operator holds your bid. Payment is
shielded ZEC, and the result is still verifiable.

**Why the settlement time is acceptable.** These are batch sales that close
once: a month's sponsor slots, a block sale, a round of seats. Bidding takes
minutes to days; settling in 15–20 minutes after the close, with every output
proven, fits that rhythm. Clearing itself, the moment the price is known, takes
about 9 seconds.

**What running one takes.** A seller and three independent operators (in the
example, the newsletter, a community foundation and an auditor), each with a
laptop-class machine. In a deployment each bidder runs the DarkCall client and deposits from
their own Zcash wallet, with the signed claim in the memo; in this demo one local
test wallet funds all four tickets. The operators never see a bid, and anyone
can check the outcome from the public bundle. Today this runs on public testnet;
mainnet follows NU7 activation on mainnet. Four bid slots per auction is the
size this prototype's circuits are compiled for.

## What is different

Some designs let everyone check the price by opening the bid amounts at the
close. Others keep them from the public, but an operator decrypts them after the
close to compute and prove the price. DarkCall proves the price without anyone
opening them, then pays out in shielded ZEC.

| | **DarkCall** | Open on-chain auction | Commit–reveal or published viewing key | Operator decrypts after the close, then proves (timelock or key committee) |
| --- | --- | --- | --- | --- |
| Bids hidden from the public | **Yes, never opened** | No | Only until the close | Yes |
| Bids hidden from whoever runs it | **Yes, from any single operator** | No | No | No, it decrypts them after the close |
| Anyone can check the price | **Yes, by zero-knowledge proof** | Yes | Yes, by opening every bid amount | Yes, by its proof |
| Payments and refunds stay private | **Yes, one shielded transaction** | No | Possible, if paid shielded | Possible, if it pays shielded |
| Payout provably follows the result | **Yes, six output proofs** | Yes, by contract | By contract, or audited afterwards | Only if the payouts are proven too |
| Audit without revealing bids or keys | **Yes, proofs only** | No, nothing is hidden | No, opens the bids or publishes a key | For the price; the operator reads every bid |

Common designs compared, not specific projects. "Any single operator" is the
REP3 bound: two colluding operators could reconstruct bids (see
[Trust model and limits](#trust-model-and-limits)).

**Auditable without revealing.** Blind auctions keep the bids secret but ask
everyone to trust the seller's result. Reveal-to-audit designs win the check back
by opening the bids or publishing a viewing key. DarkCall keeps the bids secret
and still lets anyone check. No bid reveal, no published viewing key, and no
operator reconstructs the bid book: anyone verifies the clearing price and every
shielded payout from public zero-knowledge proofs, without decrypting a bid.
Two bid books for three slots, 77 · 61 · 43 · 21 and 22 · 22 · 22 · 21, give the
same result (price 21, three sold, the same refunds). A commit–reveal or
viewing-key audit shows which one it was, and next month's floor moves to match.
DarkCall's public proofs tell a verifier the same thing for either book (the bid
commitments differ but reveal no amount), and no single operator learns which
book it was. Who needs a viewing key:

| Who | Viewing key | What it gives them |
| --- | --- | --- |
| Anyone verifying | None | The price and every payout, checked from proofs, signatures and the transaction; no bid or refund decrypted |
| Each bidder | Its own, kept | Its own encrypted receipt; the key is never handed to anyone, verifiers included |
| The three operators | The escrow's viewing key | Admitting deposits: fixed-size deposits, never a bid. They learn winners and refunds; none alone can rebuild the bid book (REP3), though knowing the winners they can tell whose bid the price is when only one bid lost |

No viewing key is ever published. In this demo every party runs on one host, so
each key staying on its owner's own device is the deployment model, not
something the recorded run shows.

What is public and what is not:

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

## Design choices

The questions a reviewer is likely to ask first, answered from the design.

**Why not one prover with a zero-knowledge proof?** A ZK proof hides the bids
from the verifier, not from the prover, who must hold every bid to build it.
DarkCall's three operators generate the proof together from secret shares
(coNoir over REP3), so the proving step never gathers the bid book.

**Why not decrypt after the close and prove the result?** Timelock or
committee decryption keeps bids sealed until the close, but whoever decrypts
then reads them all. DarkCall never reconstructs a bid, before or after.

**Why not publish a viewing key, or reveal the bids, for audits?** Either
makes every amount readable. DarkCall's audit is the proofs: verifying needs no
key and no bid.

**Why two proof systems?** coNoir proves on BN254 (UltraHonk, checked by
Barretenberg), while Zcash notes are commitments on Pallas, so the auction proof
cannot speak about the notes directly. Each Halo2 proof recomputes one payout
row's BN254 Poseidon2 hash inside a Pallas circuit and opens the matching note
commitment, which ties the two without a trusted bridge.

**Why REP3, which is only semi-honest?** It is the honest-majority, three-party
setting the pinned coNoir proves in, and the joint clearing proof takes seconds.
A malicious-secure version is not claimed: two colluding operators could
reconstruct the bids, and the trust model says so.

**Why 2-of-3 FROST rather than 3-of-3?** One operator alone cannot move the
escrow, and one stopped operator cannot strand it: the other two can still
refund every deposit (shown on regtest, [record](../evidence/2026-10-24-cancel-refund.json);
not offered on public testnet yet).

**Why a uniform price, and why four bids?** Every winner pays the same price,
the highest losing bid or the reserve, so that one amount follows from the
public price; it is disclosed rather than hidden. The circuits have four bid
slots and six outputs; clearing was measured at eight bids, and the settlement
cost grows with one Halo2 binding per payout.

## Settled on public Zcash testnet

On October 19, 2026 an auction was **created, funded, cleared and settled by
clicking through the browser UI on standard public Zcash testnet (NU7)**, with
testnet coins and the same proofs, admission and signing as below. It is the
settlement re-checked in the video and under "See a settled auction":

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
root. Full record: [testnet evidence](../evidence/2026-10-19-public-testnet-settlement.json).
`./demo.sh --chain testnet` runs Fund and Settle on testnet from the browser.

**Again on the live demo, Oct 27.** A new auction went through the hosted site
at [https://49-12-250-128.sslip.io](https://49-12-250-128.sslip.io) on a Linux server, every click as a visitor makes it: deposit
[`76329a49…3835245`](https://testnet.zecblock.com/tx/76329a49cc6e7709cf559c609a7f42135463aab4659f7d1db88462a9e3835245)
(block 4,548,955), clearing 6.8 s, settlement
[`03feb02b…9cf72d8b`](https://testnet.zecblock.com/tx/03feb02b365a0f2a3df1bef3b338fdcd8689e2081981b0f896ad95269cf72d8b)
(block 4,548,995) after six output bindings and 2-of-3 FROST signing, public
check 17.6 s, both receipts opened and a corrupted receipt proof refused:
24 min 16 s from the landing page. Its bundle verifies with
`make verify BUNDLE=hosted-testnet-auction-nu7-v1`
([record](../evidence/2026-10-27-hosted-testnet-auction.json)).

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
[browser settlements](../evidence/2026-10-18-fresh-book-native-settlement.json),
[recorded settlement](../evidence/2026-10-05-canonical-funded-nu7-memo-auction.json).

<table>
<tr><td><img src="../demo/media/v2/04-sealed.jpg" alt="Four sealed tickets with the six-step bar"></td>
<td><img src="../demo/media/v2/07-funded.jpg" alt="Deposits locked on Zcash regtest and admitted over the same root, before clearing"></td></tr>
<tr><td>Four sealed tickets: each a client ZK proof plus three operator signatures; no value shown.</td>
<td>Fund on Zcash: real shielded deposits, admitted over the same root, before any clearing.</td></tr>
<tr><td><img src="../demo/media/v2/16-settled.jpg" alt="Settlement stages done, who learned what, and the settlement transaction"></td>
<td><img src="../demo/media/v2/20-receipt-tamper.jpg" alt="A losing ticket's shielded refund decrypted, and a corrupted receipt proof rejected"></td></tr>
<tr><td>Settle: MPC delivery, six output bindings, FROST signing, confirmations, public check.</td>
<td>Each ticket opens only its own shielded receipt; a corrupted proof is rejected before the key is read.</td></tr>
</table>

Frames from the video run on public testnet (auction `4dde6fcb…`, settlement `be1157c8…a1c8ad67`); see
[demo/media/v2/manifest.json](../demo/media/v2/manifest.json).

## Try it

With the pinned tools built ([reproduce.md](run/reproduce.md)):

```sh
make doctor   # are the pinned tools, bundles and disk ready?
make verify   # public verification of the recorded testnet settlement, no secrets
make demo     # the local demo at http://127.0.0.1:8877
```

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
each; [record](../evidence/2026-10-18-clean-room-linux-verification.json)). Most
of those 100 s rebuilt the fixed Halo2 parameters; they are now cached behind a
digest pinned in the source, and on Oct 21 the eight committed bundles verified
on the host in about 19 s each, the first check on a machine taking about
1.5 min longer ([record](../evidence/2026-10-21-halo2-params-cache.json)). Remove
the image afterwards with `docker image rm`.

**Or verify on the host.** Build the pinned tools
([reproduce.md](run/reproduce.md)), then run:

```sh
./verify.sh --funded demo/artifacts/browser-auction-nu7-v2 \
  --authority demo/authority/browser-auction-nu7-v2.json
```

**Try to cheat it.** With the pinned tools built, this makes one dishonest
change at a time to a copy of the recorded testnet settlement and runs the same
verifiers on each copy: one accepted bid's commitment changed in the signed
book, a price one higher in both auction statements, and two payout proofs
swapped so each is checked against the other output. A change counts as
refused only when the check it targets refuses it, and an unchanged control
copy must pass. In the latest run the control passed in 19 s and all three
were refused: the bid by the signed book in 2 s, the price by the auction proofs
in 19 s and the swapped proofs by the output bindings in 16 s (the first
recorded run took 2, 102 and 97 s, before the fixed Halo2 parameters were
cached; [record](../evidence/2026-10-21-cheat-lab.json),
[rerun](../evidence/2026-10-21-halo2-params-cache.json)). These change proofs and
signed data, not the transaction: a quorum of operators that signed a
different transaction is outside this test. The [live demo's](https://49-12-250-128.sslip.io/demo/settled)
**See a settled auction** page has the same buttons.

The same page's **Inspect the proof trail** prints the public data behind each
claim with 35 consistency checks between the bundle's files and the transaction bytes; the same from the shell:

```sh
python3 -B scripts/proof_trail.py demo/artifacts/testnet-auction-nu7-v2 demo/authority/testnet-auction-nu7-v2.json
python3 -B scripts/cheat_lab.py none book price payout
```

**Run the whole product locally.** You need the pinned tools, Docker, and the
local NU7 regtest node and test bank ([regtest.md](run/regtest.md)).

```sh
python3 scripts/regtest_nu7_node.py start
./demo.sh            # http://127.0.0.1:8877
```

Then: **Run an auction → Create auction** (default terms) → seal four demo bids →
wait for the close → **Certify closed book → Fund on Zcash** (real deposits and
chain admission, about 2 minutes) **→ Clear this auction → Open my result →
Settle on Zcash** (about 10 minutes on the local regtest on a 16 GiB M1, with live progress). After it finishes, **Open my shielded receipt** decrypts your
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
  Clearing needs all three operators. If one stops before settlement, the other
  two refund every deposit to its certified refund address, less a 5,000 zatoshi
  share of the network fee, in one transaction signed 2-of-3 with FROST; the
  stopped operator's key share and journal are not needed. Shown on regtest only
  ([record](../evidence/2026-10-24-cancel-refund.json)); auctions on public testnet,
  including the live demo's, do not offer it yet. A refund carries no auction
  proof, since there is no result: each bidder sees its refund in its own wallet.
  If two operators stop, the deposits wait in escrow.
- **Operators see outcomes.** The three operators hold the escrow's viewing key,
  which admitting deposits needs: it shows them fixed-size deposits, not bids, and
  they learn refunds and winners. No viewing key is published, and public
  verification needs none.
- **The price is one bid.** Under these rules the public clearing price equals
  the highest losing bid, or the reserve. That single value is public by design;
  who placed it is not, except to the operators, who know the winners and so can
  tell whose bid it is when only one bid lost (as in the four-bid, three-unit
  demo). With one unit there are three losers, and knowing the winner no longer
  names the bid that set the price. Small books and repeated auctions allow
  further inference.
- **The bidder's client runs as a local service.** Only the bidder's own client sees the
  typed bid; operators receive shares and a proof. In this demo that client is a
  localhost service on the same machine as the operators. Deposits come from one local test wallet right
  after the close; in a deployment each bidder's own wallet deposits. All
  three operators run on one machine.
- **The book is complete only by the operators' word.** The public check proves
  the result over the operators' signed book; it cannot show that every funded
  bid made it in. Each bidder holds all three operators' signed acceptance and
  checks that its own ticket is in the book, so dropping an accepted bid is
  visible to that bidder, not to outsiders.
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
  settlement path takes one to three units with budget 100 and reserve 0 or 20: a
  one-unit auction (price 61, one sold) settled on public testnet on Oct 21, tx
  [`1d85d98f…82db`](https://testnet.zecblock.com/tx/1d85d98f15691b06337f791d3d470cfd04de261f0ab057f763663ffc01d282db).
  Four is the circuit size, not a speed limit: see [beyond four bids](#beyond-four-bids).
- **Demo-sized amounts.** Bids stay within a 100-zatoshi budget, and the fixed
  15,000-zatoshi processing fee per ticket pays the settlement's 30,000-zatoshi
  network fee and a 30,000-zatoshi operator fee output. The numbers exercise
  the cryptography end to end; they are not a pricing proposal.

An earlier delivery design leaked refunds to a two-candidate guess. We reproduced
the attack, withdrew the claim and replaced it with blinded v2 commitments:
[attack and fix](design/operator-fill-delivery.md).

## Measured on a shared 16 GiB Apple M1

| Scope | Time |
| --- | --- |
| Client bid: input ZK proof + shares + 3 operator signatures | 0.5–0.6 s |
| Fund on Zcash: deposits + chain admission + certificate (includes a 120 s admission window) | 121 s |
| Browser REP3 clearing incl. coNoir proof and verification | 7.6–9.4 s |
| Native settlement of a fresh book, uninterrupted (2 threads), Halo2 parameters cached | 561 s (1 unit), 623 s (3 units) |
| Same, before the cache (parameters rebuilt three times per settlement) | 885–1025 s |
| Settle → publicly verified (fund-first flow), parameters cached | 580–642 s (998–1024 s before) |
| Public-only verification of a settlement, parameters cached | 18–32 s |
| Same, on a machine's first check (computes the parameters, ~87 s) | 99–127 s |
| Own receipt opening (proofs checked before key use) | ~3 s |
| Recorded Oct 5 settlement (1 thread, memory pressure) | 1848 s |

These are single-host measurements, not a controlled benchmark.

### Beyond four bids

`scripts/measure_bid_scaling.py` derives N-bid versions of the three coNoir
circuits (only the bid count changes; the four-bid versions match the committed
circuits gate for gate) and runs the clearing proof with three REP3 parties on
synthetic bids. Record: [evidence/2026-10-21-bid-scaling.json](../evidence/2026-10-21-bid-scaling.json).

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
binding per payout (N refunds, the seller, the fee), about 80–150 s each here
after a one-time key. The rows are independent: on a Linux host with enough
memory, `DARKCALL_ROW_WORKERS=2`, `3` or `6` proves them in parallel under the
existing memory guard (checked before any deposit). That is wired but not yet
measured; this Mac proves them one after another. Beyond a few dozen bids, all-pairs ranking should give way to a
sorting network.
