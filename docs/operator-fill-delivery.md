# Operator-only refund delivery

The auction operator needs actual output amounts to build a native transaction.
It must not obtain the maximum bids just to derive those amounts. `delivery4`
uses the same committed REP3 inputs and auction rules, and returns public price,
sold count and four **masked** refunds.

For each ticket, the operator fixture samples a fresh uniform BN254 field mask
`m_i` with the cryptographic RNG. It privately shares these masks into REP3. The
circuit checks a Poseidon2 commitment to the mask, context, accepted root and
ticket, then publishes `refund_i + m_i mod q`. The operator decrypts only these
four values with its masks. Both client/share validation and normal C++ proof
verification run before the delivered values are consumed. No full witness or
maximum bid is reconstructed. This is ordinary field masking with an existing
commitment, not a new hash or a native Zcash consensus feature.

The commitment prevents the decoder from silently using a different mask or
auction. It does not prove entropy or prevent intentional mask reuse. The
implementation must generate and keep fresh masks for every delivery epoch.
Masks and decoded refunds are written with mode 0600 under ignored local fixture
directories. They must not be committed or sent to the public verifier.

Operators learn individual refunds and therefore winning status when the price
is positive. They can also use public price/count/timing to infer information.
This matches the custody/viewing metadata disclosure of this prototype; it is
not a claim that operators learn nothing about fills. Zero-price winner flags
need a separate private delivery field before supporting that case in the full
product. Current native integration fixes reserve 20, budget 100, fee 15000 and
four active tickets. Funds, receipts and independent-host deployment remain
separate gates.

The native integration consumes these delivered amounts, builds PCZT outputs,
proves the six row-to-cmx bindings, and runs a second committed MPC proof that
binds the exact native rows to the auction rules. Honest FROST signers check the
verified plan, actual output commitments and v6 effects digest before signing.
The final public verifier does not require masks or decoded refund files: the
second auction proof directly binds all six amounts to the transaction outputs.

```sh
python3 scripts/run_delivery_gate.py --book-run .local/BOOK_RUN
RAYON_NUM_THREADS=2 target/release/darkcall-native-gate \
  --integrated .local/BOOK_RUN UNUSED_LOCAL_NAME
target/release/darkcall-native-gate --verify-integrated .local/UNUSED_LOCAL_NAME
```

These commands construct fabricated input notes and anchors. Their transactions
must never be broadcast. Real wallet scanning, deposit confirmation, signed
acceptance/recipient receipts, persistent reservations and finality are unfinished.
