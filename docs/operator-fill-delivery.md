# Operator-only refund and winner delivery

Native transaction construction needs output amounts, but does not need the
maximum bids. The default `delivery_private4` circuit consumes the same
authenticated REP3 inputs and returns public price/sold count and four masked
refunds. `delivery_flags4` adds independently masked winning bits, needed when
the price is zero and every ticket has the same refund.

## Version 1 privacy defect

Historical `delivery4` commits deterministically to each mask. Given public
budget B, price p and ciphertext c, a reader can try the two possible refunds
`B` and `B-p`, compute each candidate mask and compare its commitment. Fresh
uniform masks do **not** prevent this attack. A public-input-only audit recovered
all four refund candidates from the historical local DKG delivery transcript.
The initially drafted unblinded winning-bit variant had the same two-candidate
defect; its audit recovered all four flags.

These artifacts remained in ignored local directories, and the final public
settlement bundle does not contain the delivery transcript. No external leak
or recovery of the full maximum-bid witness is asserted. Nevertheless, v1 cannot
support transcript privacy claims. Existing proof correctness/native signatures
remain reproducible. `--legacy-masks` is explicit historical reproduction only;
native construction now requires v2.

## Version 2 binding and masking

For every ticket and output kind, the operator independently samples a uniform
BN254 mask m and a uniform private commitment blinder s with the cryptographic
RNG. Both are privately shared into REP3. The circuit checks

```
C = Poseidon2(domain_v2, context, accepted_root, ticket_limbs[0..4], m, s)
c = refund + m mod q
```

Winning bits use the distinct `DarkCall/winner-mask/v2` domain, separate masks
and separate blinders. Refunds use `DarkCall/fill-mask/v2`. Context/root/ticket
binding prevents substitution. For a candidate refund or bit, an outsider can
compute a candidate mask but cannot compare a commitment without the independent
secret blinder. The hiding claim assumes sufficient fresh randomness and the
existing Poseidon2 construction, together with the stated semi-honest REP3
model. The circuit proves neither RNG entropy nor intentional non-reuse.

The operator opens refunds/bits using its pads only after client/share checks,
coNoir verification and independent normal C++ Barretenberg verification against
the compiled fixed circuit. No full witness or maximum-bid list is reconstructed.
Pads include both mask and blinder; they are mode 0600. Gate directories are
mode 0700 and inherit umask 077, including upstream-generated secret shares.
Never include pads, witnesses, shares or decoded amounts in public artifacts.

Actual v2 REP3/coSNARK runs pass for nonzero-price refunds and zero-price flags.
Normal C++ verification matches the trusted keys and rejects price/root/cipher
changes and changed flags. A candidate audit with 16 small blinder guesses finds
no matches: this is a regression test, **not** an exhaustive or formal hiding
proof. The native delivery consumer also passes with verified v2 refunds. See
[recorded evidence](../evidence/2026-10-05-blinded-delivery-and-dictionary-attack.json).

## Remaining disclosure and integration scope

Operators learn individual refunds and therefore winning status when price is
positive. Public outcomes may imply other information: for four eligible bids
and three units, identifying the loser reveals that its maximum equals the public
K+1 price. Zero-price refunds are all B and are already implied by the rules;
separate flags distinguish winners. Masking cannot hide consequences of the
public output. No IP/timing protection or malicious-MPC security is claimed.

Native integration fixes budget 100, processing fee 15000, three units and four
active tickets. Reserve 20 uses v2 refund delivery; reserve zero requires v2
refund-and-winner delivery. After independent C++ verification, the native
consumer re-derives a fresh opening directly from the exact transcript and private
pads, checks the winner count/refund relation and conserves the output total.
Decoded winner bits remain private; summary files expose only whether verification
passed. Price-zero/sold-three execution also passes through six fresh bindings,
MPC, PCZT, four FROST spends, native consensus proof/signatures and all six
decryptions in 742.670s with fabricated funding. Its public replay passes in
103.898s, including rejection under the other valid compiled layout/VK. This
does not establish funded-chain zero-price execution or bidder-status receipts.
See [zero-price evidence](../evidence/2026-10-05-zero-price-native-v2.json).

A **fresh funded v2 auction** now completes new local DKG escrow deposits, 12 signed
receipts, three final-book endorsements, six fresh native row proofs, committed MPC,
four FROST spends, chain acceptance, two controlled-chain confirmations and six
exact recipient decryptions. Its price is 25/sold 3, distinct from the historical
price-20 book. Public-only certified replay passes in 100.871s and rejects ten
certificate/book/recipient/layout changes. No escrow master key or maximum-bid list
is read. All participants share one host on an owned isolated regtest; full
public-testnet/NU7 wallet migration and funded zero-price remain separate gates.
See [fresh funded evidence](../evidence/2026-10-05-fresh-funded-blinded-v2.json).

The native integration uses delivered amounts to construct PCZT outputs, proves
all six row-to-cmx bindings and verifies a second committed MPC proof tying the
actual rows to auction rules. FROST signers inspect the verified plan, exact
outputs and effects digest. Public final verification needs no delivery pads or
transcript: its separate settlement proof binds all six amounts to actual cmxs.
Fully cached recovery verifies all six proofs with the original fixed VK before
reusing them; it does not construct an unnecessary proving key.
New explicit shared18 runs use their own compiled fixed VK. Present malformed
metadata fails closed; only genuinely absent metadata denotes the historical
working33 circuit. Neither path accepts a bundle-supplied key.

```sh
python3 scripts/run_delivery_gate.py --book-run .local/BOOK_RUN
python3 scripts/run_delivery_gate.py --book-run .local/ZERO_PRICE_BOOK --winner-flags
RAYON_NUM_THREADS=2 target/release/darkcall-native-gate \
  --integration-delivery-check .local/BOOK_RUN UNUSED_LOCAL_NAME
```

The delivery-only commands construct no native transaction and do not broadcast.
`--integrated` constructs fabricated inputs/anchors and must never be broadcast.
Actual controlled-chain funding uses the separate regtest driver, which checks
real owned funding/witnesses, signed receipts, reservations and confirmations.
