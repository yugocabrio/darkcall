# DarkCall protocol

This is the protocol as implemented and exercised in the recorded runs (public
Zcash testnet, NU7, Oct 19–27, 2026). For each stage it says who sends what, who
learns what, what is proven and what is trusted. Anything that has not run is
marked as such. The earlier design proposal, written before development, is kept
as history in [protocol-ja.md](protocol-ja.md) (Japanese).

Three kinds of check appear below. Keep them apart:

- **The auction proofs** (coNoir, UltraHonk on BN254) prove the auction rule over
  the committed bids. Anyone verifies them with Barretenberg.
- **The six output-binding proofs** (Halo2 on Pallas) prove that each output of
  the Zcash transaction pays the amount and recipient of one proven payout row.
- **Zcash consensus** validates the native transaction: its proof, its spend
  signatures and its value balance. It knows nothing about the auction and does
  not enforce its rule.

## Roles and keys

| Party | Holds | Never holds |
| --- | --- | --- |
| Seller | The auction terms and its payout address | Any bid |
| Bidder (one per ticket) | Its maximum bid, salt and share randomness; its refund address and that address's viewing key | Other bids |
| Operator A, B or C | An Ed25519 application key (acceptance receipts, the accepted book); one REP3 party role; a 2-of-3 FROST key share of the escrow (RedPallas, made by a DKG, no master key); the escrow's full viewing key | A bid in the clear, or the escrow spending key |
| Public verifier | The auction's trusted manifest: conditions and the three operators' application keys, obtained independently of any bundle | Any key or secret |

Application keys and FROST custody keys are separate. An auction has four bid
slots and six payout outputs.

## The auction rule

Terms: units K (1 to 3), reserve r (0 or 20 on the native path), budget B = 100
zatoshi and processing fee f = 15,000 zatoshi per ticket, all fixed in a
conditions digest. Bids at or above r are eligible and ranked by amount, ties
broken by ticket ID. The top K win. If more than K bids are eligible, the price
is p = max(r, the (K+1)-th bid); otherwise p = r. Every winner pays p.

| Output | Amount |
| --- | --- |
| Each winner's refund | B − p |
| Each loser's refund | B |
| Seller | sold × p |
| Operator fee | 4 × f − network fee |

The network fee is 5,000 zatoshi per action, 30,000 for the settlement's six
outputs. Example: bids 77, 61, 43, 21, K = 3, r = 20 give p = 21, refunds 79,
79, 79 and 100, seller 63, operator fee 30,000; with the network fee that is
exactly the four deposits of 15,100.

## 1. Seal a bid (bidder's client → operators)

**Sends.** The client fixes a context `ctx = H(DarkCall/ctx/v1, conditions,
K, r, B, f)`, where H is Noir's Poseidon2 sponge over BN254, and commits to its
bid x with a random salt s: `c = H(DarkCall/bid/v1, ctx, ticket, x, s)`. It
splits x and s into three additive shares (`x = x₁ + x₂ + x₃`) and sends
operator i the pair (xᵢ, xᵢ₋₁) with the matching salt shares, plus the
commitment and a client proof.

**Proven.** The client proves in Noir (`bid_input`, 4,580 gates, UltraHonk) that
the commitment and the three pair hashes open to the same x and s, and that
x ≤ B. Each operator recomputes its own pair hash, checks the proof with the
key derived from the agreed circuit, and signs an acceptance receipt for the
ticket (twelve receipts for four tickets).

**Learns.** Each operator sees two of three random shares, which say nothing
about x on their own.

**Trusted.** The client runs on the bidder's own device and draws fresh
randomness. In this demo every client runs on the demo's server, which therefore
sees the typed bid.

## 2. Fund and admit (bidders → escrow, operators sign the book)

**Sends.** Each ticket pays the same D = B + f = 15,100 zatoshi to the escrow's
shielded Ironwood address, with a memo that binds the payment to the network,
the auction conditions, the ticket ID, a fresh client key and a nonce. Equal
deposits say nothing about bid size. In the demo one test wallet funds all four
tickets in one transaction.

**Checked.** Each operator decrypts the deposits with the escrow's viewing key,
checks amount, memo and two confirmations, re-verifies the four client proofs
and its share pairs against the deposits, and journals the admission to disk
before acting on it. At the close all three sign the accepted book: a Poseidon2
root over the context and the four ticket leaves, with a certificate that binds
every ticket, bid commitment and refund-address commitment, and the seller and
operator-fee commitments.

**Why the book is signed.** An operator who wanted another price could quietly
drop an inconvenient bid. The proofs below are made over this signed root, and
each bidder holds all three operators' receipts for its own ticket, so it can
show that its ticket was dropped. Outsiders cannot see a dropped bid.

**Trusted.** Funding is attested by the operators' signatures. It is **not** a
public zero-knowledge funding proof. Chain observations come from the operators'
node or, on testnet, one lightwalletd provider.

## 3. Clear (operators, jointly)

The three operators run the clearing circuit as REP3 multi-party computation on
the shares, and coNoir turns the computation into one UltraHonk proof that the
normal Barretenberg verifier accepts. The circuit opens the committed leaves
inside MPC, checks them against the signed root, applies the auction rule and
outputs the price and the number sold. The demo's clearing step runs
`delivery_flags4` (11,894 gates), which also produces the private delivery below;
`auction4` (10,247 gates) is the same rule without delivery. It takes under
10 seconds (6.8 s on the hosted server).

**Learns.** Only what is public: the price and the number sold. No party
reconstructs a bid, and there is no single prover.

**Trusted.** At most one operator is corrupt, and it is semi-honest (REP3). Two
colluding operators can rebuild every bid. All three must take part.

## 4. Deliver results privately (MPC → operators)

To build the payout, the operators need each refund, not the bids. For every
ticket the circuit outputs a masked refund `refund + m` and a commitment
`H(domain, ctx, root, ticket, m, b)` to a fresh mask m and an independent blinder
b, and the same for a winner bit. After verifying the proof, the operators open
these with their private pads.

**Learns.** The operators learn each refund and so who won. When only one bid
loses, as in the demo, that tells them the losing bid was the price.

An earlier version committed to the masks without the blinder, which let anyone
guess each refund from two candidates. We reproduced the attack and replaced
the scheme: [operator-fill-delivery.md](operator-fill-delivery.md).

## 5. Prove the payout plan (operators, jointly)

Six payout rows of 224 bytes each fix the domain, the conditions, the accepted
root, the ticket (zero for the seller and the fee), the role, the native action
index, the recipient (gd, pkd), the amount and a fresh salt
([settlement-rows.md](settlement-rows.md)). The operators prove jointly with
coNoir (`settlement_poseidon4`, 24,016 gates) that these rows follow the auction
rule from the committed bids. Its statement has 232 public fields. The fixed
inputs are the conditions, the four ticket IDs, the terms, the four bid
commitments, the signed root, six receiver commitments and six action indices.
The outputs are the price, the number sold, a plan commitment and the six row
hashes h₁ to h₆.

## 6. Bind each output to its row (Halo2)

For each row one Halo2 proof on Pallas (K = 17) shows, for the same private row
and note opening:

```
Poseidon2_BN254(rowᵢ) = hᵢ                    # the row the auction proof output
NoteCommit(gd, pkd, v, ρ, rseed) = cmxᵢ         # the note at action aᵢ of the transaction
```

Its public inputs are hᵢ, cmxᵢ and aᵢ. BN254 arithmetic is emulated inside the
Pallas field, so the two proof systems are tied without a trusted bridge
([output-binding.md](output-binding.md), [non-native-poseidon.md](non-native-poseidon.md)).
Not constrained here: the ZIP 2005 derivation of rseed and the note ciphertext.
Signers check the transaction's outputs directly, and each recipient checks its
own note by decrypting it.

## 7. Authorize and settle (any two operators → Zcash)

The operators build one NU7 (v6) transaction: four Ironwood spends of the
deposit notes and six outputs (four refunds carrying encrypted receipts, the
seller, the operator fee). Each operator journals the reservation of that one
transaction before signing, so a journal that settled cannot also refund, and
the reverse. Any two operators then sign the four spends with 2-of-3 FROST, each
only after checking the auction proofs with the normal verifier, the six
bindings against the actual outputs and action indices, the accepted book's
three signatures, and the transaction's effects. The network validates the
transaction; the run waits for two confirmations.

**Trusted.** Two colluding signers can sign a different payout. It would fail
public verification, but it is not prevented.

## 8. Open your own receipt (bidder)

Each refund note carries a 512-byte memo with the ticket ID, the accepted root,
and that ticket's refund and winner masks and blinders: no bid and no plain
result. The bidder's client first verifies the `delivery_flags4` proof against
the signed book. Only then does it use the bidder's own viewing key to decrypt
the memo and open its refund and winner bit. A corrupted receipt proof is refused before the key is read.

## 9. Verify publicly (anyone)

The public bundle holds the transaction bytes, the accepted-book certificate,
the two coNoir proofs with their public inputs and fixed verification keys
(`settlement_poseidon4`, `delivery_flags4`) and the six Halo2 binding proofs.
It holds no bid, share, mask, refund, key or PCZT. `verify.sh --funded BUNDLE
--authority MANIFEST` checks, in about 20 seconds:

1. the three operator signatures over the accepted book, against keys from the
   trusted manifest, never from the bundle;
2. that the 232 public fields match the certificate and the terms;
3. both coNoir proofs, with pinned Barretenberg and fixed keys;
4. the transaction: native proof and spend signatures, and the cmx and action
   index of each output, read from its bytes;
5. the six Halo2 bindings against those outputs.

It does not show chain inclusion (the bundle alone cannot), that funding
happened (attested, item 2 above), or that every funded bid reached the book
(each bidder checks its own). Changing a bid, the price or a payout proof is
refused by the check it targets: [cheat lab record](../../evidence/2026-10-21-cheat-lab.json).

## If something fails

| Case | What happens |
| --- | --- |
| One operator stops before settlement | Clearing needs all three, so the auction cannot settle. The other two refund every deposit in one transaction, 10,100 of 15,100 zatoshi each, signed 2-of-3 without the stopped operator's key share. **Shown on regtest only**; not offered on public testnet ([record](../../evidence/2026-10-24-cancel-refund.json)). |
| Two operators stop | Nobody can sign; the deposits wait in escrow. |
| A crash during settlement | The operators resume from their journals with the exact prepared transaction; there is no second payment ([record](../../evidence/2026-10-05-dkg-funded-resumed-auction.json)). |

## Security limits

- Bid secrecy holds against one semi-honest operator (REP3), not malicious MPC
  and not two colluding operators.
- Custody is 2-of-3: two colluding signers can move escrowed funds. Bidders have
  no unilateral refund.
- Funding and the completeness of the book rest on the operators' signatures.
- The operators learn every refund and who won; the public price is the highest
  losing bid or the reserve.
- Zcash consensus does not enforce the auction rule; signers and public
  verifiers check it.
- The demo runs every role on one server, including each bidder's client, and one
  test wallet funds all four tickets. Four bid slots; public testnet only.

## In the code

| Stage | Where |
| --- | --- |
| Client proof and shares | `circuits/bid_input`, `crates/input-tool` ([input-commitments.md](input-commitments.md)) |
| Admission, journals, accepted book | `crates/wallet-protocol`, `crates/wallet-bridge` ([accepted-root.md](accepted-root.md), [wallet-bridge.md](wallet-bridge.md)) |
| Clearing and delivery | `circuits/auction4`, `circuits/delivery_flags4`, `circuits/protocol_lib` |
| Payout plan | `circuits/settlement_poseidon4` |
| Output binding | `crates/output-binding` |
| Transaction, FROST, receipts, verifier | `crates/native-gate`, `tools/nu7-compat` ([native-gate.md](native-gate.md)) |
| Orchestration | `scripts/rehearsal.py`, `scripts/settle_fresh_book.py`, `scripts/serve_demo.py` |
