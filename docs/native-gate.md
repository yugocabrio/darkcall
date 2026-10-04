# Offline native transaction gate

Run with the locked toolchain:

```sh
cargo test --locked -p darkcall-native-gate
cargo run --release --locked -p darkcall-native-gate -- --prove
cargo run --release --locked -p darkcall-native-gate -- --prove --zero-rows
```

The program fabricates four Ironwood V3 input notes and a consistent four-leaf
Merkle tree. It uses the plaintext oracle to request four refunds, one seller
payment and one operator payment. The native Zcash builder, standard ZIP 317
fee rule, PCZT Creator and IO Finalizer build a v6 Ironwood-only transaction.
This is a synthetic correctness experiment; its notes and anchor exist in no
chain. These transactions must never be broadcast.

A fresh synthetic escrow key is split by a trusted dealer into 2-of-3 FROST
shares. Exactly two shares authorize each real spend with fresh per-action
nonces and the PCZT's native randomizer. The group key must equal the note's
spend validating key. A single share fails aggregation. A changed transaction
sighash fails native signature verification. Setup is in one process, without
an independent-host custody claim or deployed DKG.

Before low-level signing, the full Verifier checks value commitments, note
commitments, nullifiers, randomized keys and cross-address policy. With `--prove`,
the native PostNu6_3 proving key generates the six-action proof, and Transaction
Extractor verifies proofs and signatures before returning the transaction.
Every recipient decrypts its assigned output and checks amount, receiver,
V3 format and commitment against the final transaction.

The initial proved run completed in 14.69 seconds after compilation: 6 actions,
21,790 transaction bytes, 16,352 native proof bytes, standard fee 30,000 zatoshi,
and all 6 recipients decrypted. See the public summary under `evidence/`.
No secrets, PCZT or transaction witness is saved by the program.

`--zero-rows` creates no sale and sets total processing fees equal to the network
fee, so seller and operator each get an explicit zero-value slot. The fast
native tests demonstrate 6 actions and decryption of all slots even in this
case. The proved zero-row run also passed transaction extraction and all 6
decryptions, retained 21,790 bytes and completed in 10.20 seconds after compilation.

Funding, testnet broadcast, signed acceptance receipts and MPC plan binding
remain separate unfinished gates. Native validity by itself does not establish
that the transaction implements an auction result.
