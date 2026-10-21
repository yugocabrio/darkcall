# DarkCall — review guide

DarkCall runs sealed-bid, uniform-price auctions on Zcash. Bidders' maximum
bids are secret-shared among three operators and are never reconstructed. The
operators compute one clearing price with REP3 MPC and prove it with a coNoir
zero-knowledge proof. One shielded NU7 transaction pays the seller and refunds
every bidder; six Halo2 proofs bind its outputs to the proven result. Anyone can
verify the public bundle without secrets. Each bidder decrypts only their own
receipt memo.

In short: blind auctions keep the bids secret but ask everyone to trust the
result; reveal-to-audit designs open the bids, or publish a viewing key, to
check it. DarkCall keeps the bids secret and still lets anyone check, with no
published viewing key and no operator reconstructing the bid book. The landing's
"Auditable without revealing" block shows the three side by side, and two bid
books (77 · 61 · 43 · 21 and 22 · 22 · 22 · 21) that give the same public
result. The worked example is a newsletter selling three sponsor slots every
month, where this month's ceilings would otherwise set next month's floor.

## What to look at

0. **The 2-minute video.** Watch [demo/media/v2/darkcall-demo-v2.mp4](../demo/media/v2/darkcall-demo-v2.mp4).
   It is edited stills from one real auction on public Zcash testnet
   (`4dde6fcb…`, settlement `be1157c8…a1c8ad67`), created and settled in the
   browser and captured by `tools/demo-capture/capture.mjs`. Every time cut
   states the real elapsed time.
1. **A settled auction, checked by you.** `/demo/settled` on the live demo. It
   is the auction in the video, and bidders are numbered as they were there.
   Start as the public (what you can and cannot see), switch to a bidder to open
   only that bidder's receipt, and read the four verified claims (recorded on
   Oct 19; re-run them here in about 20–30 s). **Inspect the proof trail** under
   them shows the public data behind each claim, read from the committed bundle
   only: the four bid commitments and the root the three operators signed, the
   auction proof's public inputs (price 21, sold 3, terms), each payout row's
   hash, action and cmx with its Halo2 proof, and the transaction, with 35
   consistency checks recomputed between those files and the transaction bytes. Then try to cheat, starting with
   **Change one accepted bid** (a few seconds): each change is made to a copy of
   the recorded bundle and checked by the same fixed verifiers
   (`scripts/cheat_lab.py`), and counts as refused only when the check it
   targets refuses it. The result sets the original bundle beside your changed
   copy and shows the exact field, before and after. In the latest run an unchanged control copy passed in 19 s
   and all three changes were refused: the bid in 2 s (signed book), a higher
   price in 19 s (auction proofs), two swapped payout proofs in 16 s (output
   bindings); the first recorded run took 2, 102 and 97 s, before the fixed
   Halo2 parameters were cached:
   [evidence/2026-10-21-cheat-lab.json](../evidence/2026-10-21-cheat-lab.json),
   [rerun](../evidence/2026-10-21-halo2-params-cache.json).
2. **The product, live.** `./demo.sh --chain testnet` (or `./demo.sh` on the
   local regtest), then **Run an auction**. Create an
   auction, seal four demo bids, certify the book, then **Fund on Zcash**: real
   deposits and chain admission over the same root. Clear it by MPC and open
   your result. Then **Settle on Zcash**: one shielded transaction,
   bound output by output to the result and publicly verified. Finally open
   your own shielded receipt and try the tamper check. Plan on about 30 minutes
   on public testnet (funding about 10, settlement 16 to 19) or about 20 on the
   local regtest; **See a settled auction** shows a finished run at once. See
   [local-rehearsal.md](local-rehearsal.md).
3. **Verification without secrets.** The easiest route is
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
4. **Exact records.** See
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
| Can operators see who won? | Yes. Operators learn refunds and winners and hold the escrow's viewing key, which shows them fixed-size deposits, not bids. None alone rebuilds the bid book. When only one bid loses, knowing the winners tells them that bid was the price: in the four-bid, three-unit demo, the loser's 21. |
| Who needs a viewing key? | Verifiers need none: the public check uses proofs, signatures and the transaction only. Each bidder opens its own receipt with its own key and hands it to no one. Only the three operators hold the escrow's viewing key. No viewing key is published. |
| What does the public price reveal? | The clearing price is the highest losing bid, or the reserve, so that one bid value is public by the rules. Who placed it, and every other bid, is not. |
| Can operators drop a bid? | The public check cannot rule it out: it proves the result over the operators' signed book, not that every funded bid made it in. Each bidder holds all three operators' signed acceptance and checks that its own ticket is in the book, so dropping an accepted bid is visible to that bidder, not to outsiders. |
| Who controls funds? | Two of three DKG FROST signers. No master key exists, but two colluders can move funds, and bidders have no unilateral refund. |
| Is funding proven publicly? | No. Operator signatures attest funding. The bundle proves transaction validity and output binding, not chain inclusion. |
| Who saw the demo bids? | Only each bidder's own client, which builds the proof and shares; here it is a localhost service. Operators receive shares and proofs. All operators, the test bank and the bidder wallets run on one machine. |
| Can bidders deposit from their own wallets? | The deposit format already allows it: one ordinary Ironwood payment of budget plus fee to the escrow address, with a memo binding it to the ticket (network, auction conditions, ticket ID, a fresh client key and a nonce), which a ZIP-321 request can carry. In this demo one local test wallet builds all four deposits in one transaction, and admission from separate wallets or transactions is not exercised. No consumer wallet sent NU7 testnet transactions yet: at the Oct 15 Arborist call, Zallet's NU7 testnet release and Zodl's NU7 support were still in preparation. |
| Public testnet or mainnet? | Public testnet, never mainnet. Two auctions settled on standard Zcash testnet (NU7) on Oct 19, one of them entirely from the browser (`be1157c8…a1c8ad67`, block 4,518,315); testnet chain data came from one trusted lightwalletd provider. Earlier runs used an isolated Zebra 7.0.0-rc.0 NU7 regtest with controlled confirmations. |
| How large can an auction be? | The committed circuits have four bid slots and six outputs, and the native settlement path supports the default terms: 3 units, budget 100, reserve 0 or 20. Four is the circuit size, not a speed limit. Measured: gate counts of all three coNoir circuits at 4, 8 and 16 bids, and the joint clearing proof at 8 bids in 6.5 s (3.1 s at 4). Estimated, not run: the clearing proof at 16 bids (about 30 s) and full settlements above four bids, which the per-payout Halo2 bindings dominate (N + 2 rows, about 80–150 s each here, one after another; a Linux host with enough memory can prove them in parallel, not yet measured). [Record](../evidence/2026-10-21-bid-scaling.json). |

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
| Native settlement of a fresh browser book, uninterrupted | about 10 min with the fixed Halo2 parameters cached (561 s one unit, 623 s three units, regtest, Oct 21); 885–951 s regtest and 984–1122 s testnet before |
| Settle → publicly verified | about 10 min on regtest with the parameters cached (580 s one unit, 642 s three units); before: 998–1024 s regtest, 1091–1224 s testnet (testnet not re-measured) |
| Public-only verification | about 20–30 s (18–32 s); about 2 min on a machine's first check, which computes the fixed Halo2 parameters once |
| Own receipt opening | ~3 s |

Settlement on public Zcash testnet is demonstrated (two auctions, Oct 19). No
independent operator hosts, second-machine reproduction or mainnet settlement
is claimed. Repository publication, deployment and contest submission
are the author's decisions.
