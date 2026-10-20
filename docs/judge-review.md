# DarkCall — review guide

DarkCall runs sealed-bid, uniform-price auctions on Zcash. Bidders' maximum
bids are secret-shared among three operators and are never reconstructed. The
operators compute one clearing price with REP3 MPC and prove it with a coNoir
zero-knowledge proof. One shielded NU7 transaction pays the seller and refunds
every bidder; six Halo2 proofs bind its outputs to the proven result. Anyone can
verify the public bundle without secrets. Each bidder decrypts only their own
receipt memo.

## What to look at

0. **The 2-minute video.** Watch [demo/media/v2/darkcall-demo-v2.mp4](../demo/media/v2/darkcall-demo-v2.mp4).
   It is edited stills from one real auction on public Zcash testnet
   (`4dde6fcb…`, settlement `be1157c8…a1c8ad67`), created and settled in the
   browser and captured by `tools/demo-capture/capture.mjs`. Every time cut
   states the real elapsed time.
1. **The product, live.** `./demo.sh --chain testnet` (or `./demo.sh` on the
   local regtest), then **Run an auction**. Create an
   auction, seal four demo bids, certify the book, then **Fund on Zcash**: real
   deposits and chain admission over the same root. Clear it by MPC and open
   your result. Then **Settle on Zcash**: one shielded transaction,
   bound output by output to the result and publicly verified. Finally open
   your own shielded receipt and try the tamper check. Plan on about 30 minutes
   on public testnet (funding about 10, settlement 16 to 19) or about 20 on the
   local regtest; **See a settled auction** shows a finished run at once. See
   [local-rehearsal.md](local-rehearsal.md).
2. **Verification without secrets.** The easiest route is
   `scripts/docker_verify.sh`. It builds every pinned tool from committed source
   in a clean Linux container and verifies every committed bundle plus a tamper
   case for each. On Oct 19 a fresh image built from commit `a7f67bd` verified
   all seven committed bundles, including both public-testnet runs.
   On a host with the [pinned tools](reproduce.md), run:

   ```sh
   ./verify.sh --funded demo/artifacts/browser-auction-nu7-v2 \
     --authority demo/authority/browser-auction-nu7-v2.json
   ./verify.sh --funded demo/artifacts/canonical-nu7-funded-v1 \
     --authority demo/authority/canonical-nu7-funded-v1.json
   ```

   The first bundle comes from an auction that was created, funded, cleared and
   settled by clicking through the browser on October 19 (tx `e3ffa436…`). The
   second is the earlier recorded settlement with fixture bids. Two more
   browser-created books from October 18 are also committed:
   `browser-auction-nu7-v1` and `fresh-book-nu7-v1`.
   Each check takes about two minutes. It needs no wallet key, PCZT, bid, mask
   or witness.
3. **Exact records.** See
   [fresh-book settlement](../evidence/2026-10-18-fresh-book-native-settlement.json)
   and [recorded settlement](../evidence/2026-10-05-canonical-funded-nu7-memo-auction.json).

The authority file is separate, caller-trusted configuration. A verifier must
obtain an auction's admission authority independently and never trust keys
chosen by the bundle producer. Manifest hashes detect packaging damage; the
pinned native, Halo2 and C++ verifiers establish validity.

## What each proof establishes

| Check | Proven by | Fails when |
| --- | --- | --- |
| A bid commitment matches its three REP3 share pairs | Client Noir proof (`bid_input`), checked at admission | Shares or commitment changed |
| The accepted book is the four admitted tickets | Three operator signatures over the root; root equals the MPC statement's root | A ticket is omitted, swapped or added |
| The clearing price, winners and refunds follow the rule | REP3 coNoir proofs (`delivery_flags4`, `settlement_poseidon4`) over committed bids | Price, plan or a row changes |
| Each transaction output pays the planned amount to the planned recipient | Six Halo2 output-binding proofs against native note commitments | Amount, recipient, cmx or action index changes |
| The transaction is authorized and valid | Native Zcash proof and 2-of-3 FROST signatures | Any byte of the effects changes |
| A bidder's own result is authentic | Fixed receipt proof plus memo opening with the bidder's own viewing key | The receipt proof or the book is altered; this is refused before the key is read |

## What is assumed

| Question | Answer |
| --- | --- |
| Can one operator see the bid list? | No, under REP3 with at most one semi-honest corrupt operator. Two colluding operators can reconstruct it. |
| Can operators see who won? | Yes. Operators learn refunds and winners and hold escrow viewing capability. |
| What does the public price reveal? | The clearing price is the highest losing bid, or the reserve, so that one bid value is public by the rules. Who placed it, and every other bid, is not. |
| Who controls funds? | Two of three DKG FROST signers. No master key exists, but two colluders can move funds, and bidders have no unilateral refund. |
| Is funding proven publicly? | No. Operator signatures attest funding. The bundle proves transaction validity and output binding, not chain inclusion. |
| Who saw the demo bids? | Only each bidder's own client, which builds the proof and shares; here it is a localhost service. Operators receive shares and proofs. All operators, the test bank and the bidder wallets run on one machine. |
| Can bidders deposit from their own wallets? | The deposit format already allows it: one ordinary Ironwood payment of budget plus fee to the escrow address, with a memo binding it to the ticket (network, auction conditions, ticket ID, a fresh client key and a nonce), which a ZIP-321 request can carry. In this demo one local test wallet builds all four deposits in one transaction, and admission from separate wallets or transactions is not exercised. No consumer wallet sent NU7 testnet transactions yet: at the Oct 15 Arborist call, Zallet's NU7 testnet release and Zodl's NU7 support were still in preparation. |
| Public testnet or mainnet? | Public testnet, never mainnet. Two auctions settled on standard Zcash testnet (NU7) on Oct 19, one of them entirely from the browser (`be1157c8…a1c8ad67`, block 4,518,315); testnet chain data came from one trusted lightwalletd provider. Earlier runs used an isolated Zebra 7.0.0-rc.0 NU7 regtest with controlled confirmations. |
| How large can an auction be? | The committed circuits have four bid slots and six outputs, and the native settlement path supports the default terms: 3 units, budget 100, reserve 0 or 20. Four is the circuit size, not a speed limit. Measured: gate counts of all three coNoir circuits at 4, 8 and 16 bids, and the joint clearing proof at 8 bids in 6.5 s (3.1 s at 4). Estimated, not run: the clearing proof at 16 bids (about 30 s) and full settlements above four bids, which the per-payout Halo2 bindings dominate (N + 2 rows, about 90 s each here, one after another; parallel proving is possible but not demonstrated). [Record](../evidence/2026-10-21-bid-scaling.json). |

An earlier delivery commitment let a public observer guess refunds from two
candidates. We reproduced the attack, withdrew the claim and replaced it with
independently blinded v2 commitments:
[attack and fix](operator-fill-delivery.md).

## Performance (shared 16 GiB M1, single host)

| Scope | Time |
| --- | --- |
| Browser REP3 clearing incl. proof + verification | ~9 s |
| Fund on Zcash, regtest (deposits, chain admission, certificate; 120 s window) | about 2 min (121 s) |
| Fund on Zcash, public testnet (10-minute window plus real confirmations) | about 10 min (612 s) |
| Native settlement of a fresh browser book, uninterrupted | about 15 min (885–951 s regtest, 984–1122 s testnet) |
| Settle → publicly verified | about 17–20 min (998–1024 s regtest, 1091–1224 s testnet) |
| Public-only verification | ~2 min |
| Own receipt opening | ~3 s |

Settlement on public Zcash testnet is demonstrated (two auctions, Oct 19). No
independent operator hosts, second-machine reproduction or mainnet settlement
is claimed. Repository publication, deployment and contest submission
are the author's decisions.
