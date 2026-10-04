# Shared settlement row encoding

The current bridge candidate uses **standard SHA-256 over exactly 224 bytes**.
Both Noir and the Halo2 native binding circuit constrain the hash. A host-computed
hash without links to the native amount/recipient is insufficient.

| Offset | Length | Field |
| --- | --- | --- |
| 0 | 16 | ASCII `DarkCall/row/v1` followed by one NUL |
| 16 | 32 | Conditions digest, unchanged bytes |
| 48 | 32 | Accepted root, canonical BN254 field encoding, big-endian |
| 80 | 32 | Ticket ID, unchanged bytes; all zero for seller/operator |
| 112 | 4 | Role, u32 little-endian: refund=0, seller=1, operator=2 |
| 116 | 4 | Actual native action index, u32 little-endian |
| 120 | 32 | Canonically compressed Pallas g_d, native point bytes |
| 152 | 32 | Canonically compressed Pallas pk_d, native point bytes |
| 184 | 8 | Amount, u64 little-endian zatoshi |
| 192 | 32 | Fresh private salt bytes |

SHA-256 consumes big-endian u32 message words. Its digest is exposed as eight
big-endian u32 words in both proof systems; no reduction modulo either field.
Padding is four blocks: byte 224 is 0x80, final eight bytes encode 1792 bits.
The expanded recipient uses the native `(g_d, pk_d)` points. Wallet addresses
must first be normalized by the pinned native SDK. The proof does not reveal
the point encodings, amounts or salts.

## Cell links and canonical encoding

`darkcall-sha-link` constrains byte ranges, fixed IV, all message/state copies,
and the SHA-256 modulo-2^32 feed-forward. The vendored Table16 API exposes raw
compression cells; the wrapper supplies padding and feed-forward.

Point x/y coordinates are the exact constrained ECC cells used by NoteCommit.
Each coordinate is decomposed into eight u32 limbs and bounded to `[0, p-1]`
using limb subtraction with boolean borrows. Recomposition alone would permit
an alternative integer differing by p. The y parity comes from a constrained
bit decomposition of its canonical low word. Compression adds the sign bit at
bit 255, then permutes the same constrained bytes into SHA message words.
The amount's two u32 limbs are equated to the native NoteValue cell.

Tests reject a field-modulus alias, a changed coordinate/sign, and a coherent
mutation of an external message word plus its bytes while the gadget's cached
values remain unchanged. The last test specifically detects missing copy links.

## Four-bid plan circuit

`circuits/settlement4` derives four refunds, the seller amount and operator amount
from the commitment-bound auction calculation. Its six public action indices
must be a permutation of 0..5. Native builders shuffle outputs: use their actual
metadata mapping, rather than assuming request order equals action order.
The six row hashes remain in canonical ticket/refund, seller, operator order.
A Poseidon2 plan root additionally commits to those hashes and result/fee/shape.

Private recipients must open the six supplied receiver commitments. Those
commitments bind context, ticket, role, compressed recipient and a private nonce.
Funding/conditions receipts still need to authenticate this receiver commitment
set. The circuit itself does not prove who issued the receipts or real funding.
This initial plan circuit accepts exactly four active tickets. General counts,
operator-only delivery of fills, admission integration and an actual MPC run of
this six-row circuit are unfinished.

## Demonstrated scope

The standalone linked Halo2 SHA-256 proof passed: K=17, 5,920 bytes, 166.42 s
observed total. A native SHA row proof also passed: K=17, 8,800 bytes, 340.01 s
including constraint/tampering checks. Both ran with other local jobs; these
are development observations, not isolated benchmarks. The native row's
conditions/root/ticket are synthetic byte strings, not an authenticated auction.

The full four-bid plan circuit compiles and passes four tests. Its shared row
library passes two encoding/hash tests including an independent Python vector.
The older portable REP3 SHA proof run exceeded its 1800-second bound. Its
witness/VK stages passed, but no complete MPC SHA proof was produced in that run.
The native CPU and ARM AES configurations are being measured separately.

Native binding retains the classical binding assumption. It does not constrain
ZIP 2005 rseed derivation or ciphertext correctness, prove funding/finality,
or establish that all six rows belong to the auction until the complete bridge
has been executed and verified. Signer ciphertext checks and recipient note
decryption remain separate requirements.

```sh
cargo run --release --locked -p darkcall-sha-link
cargo test --release --locked -p darkcall-sha-link -- --test-threads=1
cargo run --release --locked -p darkcall-output-binding --bin sha_binding
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/settlement_lib
.deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/settlement4
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/settlement4
python3 -B scripts/run_row_hash_gate.py
```

The optional `sha_binding --export-mpc-input FIXTURE_NAME` writes only synthetic
private row bytes and a public digest/cmx under ignored `.local/FIXTURE_NAME`.
`run_row_hash_gate.py --input .local/FIXTURE_NAME/Prover.toml` shares that same row.
This export never contains wallet keys or native note randomness.

`darkcall-settlement-types` implements the same Rust wire encoding and a public
statement link checker. Its independent golden vector matches Python/Noir.
The checker rejects a changed row hash, reordered native commitments, reused
or out-of-range action indices and the wrong action count. It performs structural
linking only: the caller must separately verify both proof systems and obtain
transaction commitments/finality from its trusted node. Its types do not assert
that a supplied statement has already been verified.
