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
[recorded evidence](../../evidence/2026-10-05-blinded-delivery-and-dictionary-attack.json).

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
does not establish funded-chain zero-price execution. Local per-ticket proof
receipts are now demonstrated separately below; network delivery remains.
See [zero-price evidence](../../evidence/2026-10-05-zero-price-native-v2.json).

A **fresh funded v2 auction** now completes new local DKG escrow deposits, 12 signed
receipts, three final-book endorsements, six fresh native row proofs, committed MPC,
four FROST spends, chain acceptance, two controlled-chain confirmations and six
exact recipient decryptions. Its price is 25/sold 3, distinct from the historical
price-20 book. Public-only certified replay passes in 100.871s and rejects ten
certificate/book/recipient/layout changes. No escrow master key or maximum-bid list
is read. All participants share one host on an owned isolated regtest; full
public-testnet/NU7 wallet migration and funded zero-price remain separate gates.
See [fresh funded evidence](../../evidence/2026-10-05-fresh-funded-blinded-v2.json).

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

## Private per-ticket status receipts

The operator can export one refund mask/blinder and one independently generated
winner mask/blinder for a ticket. Each recipient supplies its own expected ticket
from registration, checks the compiled `delivery_flags4` proof with the fixed
normal C++ verifier, and then opens just its own refund/winning bit. The opening
checks the exact accepted public book, ticket, root, v2 domains, commitments and
refund/price relation. No other ticket's pads or maximum bids are required. The
opening CLI reports `proof_verified_here=false`; it is insufficient without the
prior fixed-circuit proof verification.

This receipt is a **private proof opening**, not an operator signature. Its
authenticity comes from the fixed ZK proof and the recipient's expected accepted
book/ticket. It does not replace signed admission/funding certificates, identify
the bidder, prove inclusion/finality or implement secure network transport.
Sharing it reveals that ticket's refund and winning status; keep it out of public
bundles. Consequences inferred from public sold count or one's own status remain.

The local zero-price receipt run checks all four recipients independently, links
the exact book/price/sold to the completed native settlement public proof, and
passes in 104.391s including native replay. Positive-price and no-sale cases pass
in 2.114s/2.103s without native replay. Across these runs, 72 changed ticket/root/
mask/blinder/domain/extra-status cases reject. All test outcomes are synthetic.
Only the operator-side exporter reads all pads; recipient opening processes read
their own receipt and public data. See
[ticket receipt evidence](../../evidence/2026-10-05-private-ticket-status-receipts.json).

```sh
python3 scripts/check_ticket_receipts.py .local/BOOK_RUN .local/FLAGS_RUN \
  --native-bundle .local/PUBLIC_NATIVE_BUNDLE --name UNUSED_RECEIPT_CHECK
```

Native-bundle linking currently accepts the completed NU6.3 public gate. The
new NU7 six-output adapter remains unverified beyond unsigned preparation until
the full resource-guarded proof/signature run succeeds.

## Native encrypted memo transport component

The same private opening now has a canonical 512-byte memo encoding. Byte0 is
ZIP302's arbitrary-data marker `ff`; bytes1–8 are `DCTKT001`. Bytes9–40 contain
the raw ticket ID. Bytes41–200 contain five canonical big-endian BN254 scalars:
accepted root, refund mask, refund blinder, winner mask, winner blinder. The
remaining311 bytes must be zero. No explicit bid or winning bit is encoded.
Noncanonical fields, unknown versions, nonzero padding and wrong lengths reject.
The original decimal JSON format remains reproducible. `001` is the memo encoding
version, bound only to the **blinded v2** flags proof, not legacy v1 delivery.

`flags-ticket-memo-export` and `flags-ticket-memo-open` use the same argument
order as their JSON counterparts. An open still reports `proof_verified_here=false`:
the fixed `delivery_flags4` C++ proof must be verified first.

The separately pinned NU7 SDK encrypts these memos using native note encryption,
builds/proves/signs a fabricated one-input/four-output transfer and serializes it.
Each recipient then consumes only that public transaction, its own private
viewing key/expected identity, expected book/ticket and the fixed public flags
proof. Native proof/signatures are checked before decryption; exact amount,
recipient, cmx and memo opening are checked afterward. This is **a transport
component, not the auction's six-output settlement**. It does not prove the
auction row commitments or connect those transfer notes to actual funded escrow.

Zero-price and positive-price component runs pass in53.545s and55.756s. Each
checks four own-key recipients,76 memo/key/amount/identity/index/schema/transaction
substitutions and12 failed cross-recipient decryptions. Changing the serialized
encrypted memo fails both native authorization and note AEAD. All fixtures/keys
are local and synthetic; no node, network service or broadcast is used. Receiver
registration, six-output memo integration and live wallet delivery remain open.
See [memo evidence](../../evidence/2026-10-05-encrypted-ticket-memos.json).

```sh
python3 scripts/check_receipt_transport.py .local/BOOK_RUN .local/FLAGS_RUN \
  --name UNUSED_MEMO_CHECK
```

The recipient can perform every check with one command:

```sh
python3 scripts/open_ticket_memo.py .local/EXPECTED_BOOK.json \
  .local/PUBLIC_FLAGS_PROOF .local/OWN_RECIPIENT_DIRECTORY TICKET_INDEX \
  --name UNUSED_CLIENT_CHECKPOINT
```

The command freezes the three real public proof files into a new private
checkpoint, derives its own negative statements, verifies the compiled fixed C++
key/proof, then reads just the recipient's viewing identity. It authenticates the
native transaction and opens the decrypted memo against the caller's expected
book/ticket. A private result records the independently checked proof/native/
amount/status states and explicitly marks bidder registration, six-output auction
settlement and chain inclusion as unverified. No caller flag can skip proof
verification. The lower-level opening commands remain explicitly insufficient
by themselves. `scripts/check_receipt_client.py` tests this actual client path.
Four zero-price and four positive-price recipient commands pass in18.227s and
14.353s, respectively;12 uploaded-key/proof/statement/book/ticket client attacks
reject. Invalid proof stops before the viewing key is read/copied or a memo is
decrypted. An initial positive run stopped at the unchanged4GiB resource guard;
its checkpoint is preserved and a new one passed after headroom returned. See
[client evidence](../../evidence/2026-10-05-recipient-memo-client.json).
