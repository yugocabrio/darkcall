# Seller and bidder flow, with same-book settlement

The browser's **Try DarkCall** mode runs one auction from creation to a shielded
payout:

**Create → seal four bids → certify the closed book → REP3 clearing → own result
→ Settle on Zcash → own shielded receipt → tamper refusal.**

Everything after **Settle on Zcash** uses the SAME sealed tickets, bid
commitments, REP3 shares and accepted root as the browser book. The settlement
is a real transaction on an isolated Zebra NU7 regtest, built by the existing
funded drivers. It is not a recording and not a different auction.

## Start

Build the [pinned tools](reproduce.md#full-local-product), start the local NU7
node, then the demo:

```sh
python3 scripts/regtest_nu7_node.py start
./demo.sh
```

Open `http://127.0.0.1:8877` and choose **Try DarkCall**. The default terms
(3 units, reserve 20, budget 100 zatoshi) are the terms the native settlement
prototype supports, and the close time defaults to five minutes ahead. Seal four
demo bids. You can share the local link with another browser on this PC. After
the deadline, **Certify closed book**, **Clear this auction** and **Open my
result**. Then press **Settle on Zcash** and watch the five stages:

| Stage | What actually runs |
| --- | --- |
| Fund & admit | 4 shielded deposits (budget + 15000 fee each) from a local test wallet to a fresh 2-of-3 DKG escrow. Zebra accepts them; 2 confirmations; the escrow scanner decrypts the 4 notes. Operators re-verify the same 4 client ZK proofs and 12 share pairs, check each signed deposit claim, issue 12 receipts and, after a 120 s admission window, sign the final book. Its root must equal the browser book's root. |
| Re-clear by MPC | Fresh REP3 `delivery_flags4/v2` run on the same shares, with new masks/blinders, fixed C++ verification. The price/sold must equal the browser clearing. |
| Bind every output | Six Halo2 (shared18) proofs bind each settlement row (role, ticket, amount, recipient) to the native note commitment at its action index. |
| Sign & confirm | REP3 settlement proof over the six rows, certificate/statement links, then 2-of-3 FROST signing of the four escrow spends. Zebra accepts the transaction; 2 confirmations; six wallets scan their exact notes; the escrow shows all four deposits spent; three operator journals finalize. |
| Public check | The public bundle (no keys, PCZT, witnesses or pads) is verified with the separately configured authority. |

Then **Open my shielded receipt** verifies the fixed proofs and the signed book
first. Only after that does it use this ticket's own viewing key to decrypt its
memo from the settlement transaction. It shows won/lost and the refund the
wallet actually received. **Tamper with receipt** flips one proof byte and shows
the refusal happening before the key is read.

The settlement job owns the local proof lock, so other proofs wait until it
finishes. It takes about 6 minutes when warm and 25 minutes or more under memory
pressure on a 16 GiB M1.

## What is real and what is simulated

| Step | Behavior |
| --- | --- |
| Seller conditions | Bounded terms, random auction ID, conditions digest including cutoff, three fresh admission keys |
| Bid commitment / shares | Poseidon2 commitment and REP3 bid/salt pairs, fresh OS randomness |
| Input ZK proof | Noir `bid_input` circuit, pinned plaindriver `--zk`, fixed C++ verification |
| Preview admission | Three signed receipts against a **synthetic** unmined note in an offline domain, for instant feedback |
| Browser clearing | Actual three-party REP3 `delivery_flags4/v2` proof, fixed C++ verification against the signed book |
| Chain funding | **Actual** NU7 regtest deposits, made by one local test bank on behalf of the four local bidders, when settlement starts |
| Chain admission | Actual: deposit decryption, claim signatures, the same client proofs and share pairs, 12 receipts, final-book certificate over the same root |
| Settlement | Actual funded six-output NU7 transaction, Zebra acceptance, controlled confirmations, wallet scans |
| Own receipt | Encrypted memo in the bidder's own refund note, opened with only that bidder's viewing key |

The localhost client helper **sees the typed demo maximum** while it creates the
proof and shares. This is not in-browser proving or a production wallet. All
three operators, the bank and every bidder wallet run on one host. Privacy
assumes at most one corrupt semi-honest REP3 party.

The chain admission is a second admission over the same book. Its certificate
uses the chain-funded operator authority; the browser's preview certificate uses
an offline authority. In a deployment, each bidder's wallet would deposit before
admission. The demo makes the four deposits at settlement time so that a
browser auction can be settled without four separate wallets.

## Boundaries and restart

The API binds loopback only and checks Origin and a session token. It accepts
bounded JSON. Requests cannot choose paths, keys, circuits, authorities, RPC
endpoints or funding sources. Maximum bids travel only in POST bodies and the
helper's stdin, never in arguments or public replies. Private logs, keys,
shares, journals and settlement checkpoints stay under ignored `0700`
directories. The public auction state exposes only phase names, timings,
transaction IDs, counts and the accepted root.

One server session allows at most eight auctions with four tickets each. A
partial admission stops that auction and keeps its checkpoint. A completed
clearing or settlement is idempotent. A failed settlement keeps its private
checkpoint, and no automatic retry or replacement transaction is made. Reloading
the page loses the local ticket capability. Restarting the server starts a new
namespace, and old links stop working.

## Checks

Against a running local demo:

```sh
python3 -B scripts/check_rehearsal.py --port 8877 --output .local/rehearsal-check-FRESH.json
python3 -B scripts/check_rehearsal_clearing.py --port 8877 --output .local/fresh-clearing-check.json
python3 -B scripts/check_fresh_settlement.py --port 8877 --output .local/fresh-settlement-check.json
```

The last one creates a default-terms book and settles it. It checks that the
settled root, price and sold units equal the browser book and clearing, opens
all four own shielded receipts, and checks their won/refund values against the
browser results. It also checks that the following are refused: settling before
clearing, a concurrent proof, a corrupted receipt proof (before key access) and
a foreign capability. Run these checks one at a time.
