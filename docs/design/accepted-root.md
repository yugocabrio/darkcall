# Accepted receipts and final-book authentication

`wallet-bridge::final_book` supports exactly four active tickets, ordered by their
full unsigned 32-byte IDs. A trusted auction manifest contains network genesis,
conditions/terms, the pinned commitment rule, three application identities,
declared cutoff and seller/operator receiver commitments. Authentication requires
all three distinct strict Ed25519 signatures. Application keys are separate from
native FROST custody keys.

Each final endorsement checks that its complete frozen local funding roster has
exactly the same tickets/bid/refund commitments and that each payload matches a
previously fsynced acceptance receipt lock. It refuses a reorg pause, an early
close, missing funding, added tickets or changed instructions. The final-book
digest is fsynced before signing and cannot change after restart. Local journal
integrity assumes the administrator does not edit or clone its private storage.

The public certificate binds all accepted payloads, not merely their bid hashes.
Its Poseidon2 root uses the exact shared host implementation of Noir's
`protocol_lib` sponge; no new hash is introduced. Public verification checks all
232 canonical BN254 inputs, including the exact conditions bytes, all ticket
bytes, terms, bid commitments, root, four authenticated refund commitments and
two configured seller/operator commitments. It also checks the fixed action
mapping and row-word bounds. Auction and native proof verification are separate
requirements. A bidder holding an all-three accepted receipt can detect that
receipt being omitted or changed in a certified book.

Signatures attest what the operators admitted. They do not prove canonical funding,
unspentness, deadline timing or delivered-share validity to the public. Operators
must check actual chain observations, client ZK proofs, delivered pairs and cutoff
during live admission. `now_utc` is the caller's trusted clock observation; it is
not a proof of time. All-three root signing has the REP3 computation's availability
requirement and does not change 2-of-3 custody's collusion risk.

## Public replay

The authenticated mode requires an **explicit trusted manifest**, independent of
the untrusted certificate/bundle. A bundle cannot establish authority by supplying
its own keys. The legacy mode continues to report unauthenticated acceptance.

```sh
target/release/darkcall-native-gate --verify-certified-integrated \
  .local/PUBLIC_BUNDLE .local/TRUSTED_SETUP/manifest.json
```

It authenticates the precise MPC statement before expensive proof verification,
then verifies the serialized native transaction, native proof/signatures, six
native binding proofs and pinned normal C++ Barretenberg proof. C++ receives a
public-only snapshot of the exact already checked statement. No bid shares,
masks, refund plaintext, PCZT, wallet keys or journal are required.

The measured offline gate generates actual full Ironwood ciphertexts with client
intent memos, verifies four client signatures, issues twelve acceptance signatures
and three final signatures, then matches an existing MPC public statement.
**Its funding observations are fabricated.** It does not rerun live client-proof
admission, and the existing native transaction was signed before these new
certificates were created. Pre-FROST enforcement in the funded orchestration is
still required. The example never persists its fresh keys.

```sh
cargo build --release --locked -p darkcall-wallet-bridge --examples
target/release/examples/synthetic_certificate \
  .local/BOOK_RUN/book.json .local/PUBLIC_BUNDLE/mpc-public/public-input0 \
  .local/NEW_SYNTHETIC_RECEIPT_GATE
python3 scripts/replay_certified_bundle.py .local/PUBLIC_BUNDLE \
  .local/NEW_SYNTHETIC_RECEIPT_GATE/final-book-certificate.json \
  .local/NEW_SYNTHETIC_RECEIPT_GATE/manifest.json --name NEW-PUBLIC-REPLAY
```
