# DarkCall — review guide

DarkCall runs sealed-bid, uniform-price auctions on Zcash. Bidders' maximum
bids are secret-shared among three operators and are never reconstructed. The
operators compute one clearing price with REP3 MPC and prove it with a coNoir
zero-knowledge proof. One shielded NU7 transaction pays the seller and refunds
every bidder; six Halo2 proofs bind its outputs to the proven result. Anyone can
verify the public bundle without secrets. Each bidder decrypts only their own
receipt memo.

## What to look at

1. **The product, live.** `./demo.sh`, then **Try DarkCall**: create an
   auction, seal four demo bids, clear it, open your result, then **Settle on
   Zcash**. The same browser book is funded, re-admitted, cleared again by MPC
   inside settlement, paid out in one shielded regtest transaction and publicly
   verified. Then open your own shielded receipt and try the tamper check. See
   [local-rehearsal.md](local-rehearsal.md).
2. **Verification without secrets.** After building the
   [pinned tools](reproduce.md), run:

   ```sh
   ./verify.sh --funded demo/artifacts/fresh-book-nu7-v1 \
     --authority demo/authority/fresh-book-nu7-v1.json
   ./verify.sh --funded demo/artifacts/canonical-nu7-funded-v1 \
     --authority demo/authority/canonical-nu7-funded-v1.json
   ```

   The first bundle comes from an auction created in the browser and settled on
   October 18. The second is the earlier recorded settlement with fixture bids.
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
| Who controls funds? | Two of three DKG FROST signers. No master key exists, but two colluders can move funds, and bidders have no unilateral refund. |
| Is funding proven publicly? | No. Operator signatures attest funding. The bundle proves transaction validity and output binding, not chain inclusion. |
| Who saw the demo bids? | The localhost helper that builds the client proof sees typed demo bids. All operators, the bank and the bidder wallets run on one machine. |
| Public testnet or mainnet? | No. This is an isolated Zebra 7.0.0-rc.0 NU7 regtest with controlled confirmations. |
| How large can an auction be? | The circuits have four bid slots and six outputs. The native settlement path supports the default terms: 3 units, budget 100, reserve 0 or 20. |

An earlier delivery commitment let a public observer guess refunds from two
candidates. We reproduced the attack, withdrew the claim and replaced it with
independently blinded v2 commitments:
[attack and fix](operator-fill-delivery.md).

## Performance (shared 16 GiB M1, single host)

| Scope | Time |
| --- | --- |
| Browser REP3 clearing incl. proof + verification | ~9 s |
| Funding + chain admission + certificate (120 s admission window) | ~2 min |
| Native settlement of a fresh browser book | 5.5–25 min depending on memory pressure |
| Public-only verification | ~2 min |
| Own receipt opening | ~3 s |

No independent operator hosts, second-machine reproduction or public-chain
settlement is claimed. Repository publication, deployment and contest submission
are the author's decisions.
