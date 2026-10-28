# Cross-field commitment comparison

This experiment uses the **existing BN254 Poseidon2 parameters and exact fixed
sponge semantics** from pinned coNoir. It does not design a new hash and has not
replaced the demonstrated SHA bridge. Source: coNoir
467a2abbad940d40ed78f38d424cc21920988976, `mpc-core` Poseidon2 parameters.

## Encoding and native arithmetic

Pack the existing 224-byte row into eight big-endian integers: seven 30-byte
chunks and one 14-byte chunk. Each is below the BN254 scalar modulus. Fixed
length/chunk sizes make this byte packing injective. Hash the eight elements
with width 4, rate 3, capacity IV `8 * 2^64`, the same sponge used by the auction.
Expose the canonical scalar digest as eight big-endian u32 words; there is no
reduction into Pallas. The selected circuit key/hash rule must be authenticated
with auction conditions before adopting an alternative in a funded protocol.

The Pallas circuit represents BN254 elements as four u64 limbs. Every input,
quotient and remainder is range-checked and bounded to `[0, q-1]` by subtraction
with boolean borrows. Multiplication enforces eight schoolbook integer equations
for `a*b = quotient*q + remainder`. Carries are offset by `2^71` and range-checked
to 72 bits; endpoints are fixed to zero before offsetting. Each limb relation's
absolute integer magnitude stays below `2^136`, far below the Pallas modulus.
It therefore cannot be satisfied by an unintended multiple of that modulus.
Addition uses a boolean reduction quotient and the same bounded carry links.

Round constants, matrix diagonal and `x^5` reductions match the pinned existing
implementation. Source byte packing copies the actual constrained cells; reading
their cached values alone is insufficient. An adversarial floor planner changes
committed source advice while leaving the cached Values unchanged: the copy
constraint rejects it. Boundary tests cover zero, u64/u128 limb crossings,
modulus edges, noncanonical aliases and an incorrect quotient.

## Measurements so far

All measurements use synthetic fixtures on one Mac, with the SHA baseline also
running. They are neither independent-host nor production performance results.

| Gate | Result |
| --- | --- |
| SHA REP3, one 224-byte row | 574.499s proof stage; normal C++ verification passed |
| Poseidon2 REP3, same sized private row | 1.678s witness, 2.262s proof; normal C++ verification passed |
| Poseidon2 on Pallas, hash only | K17; 2.727s mock, 69.472s keygen, 39.126s proof; 6848 bytes |
| Native Poseidon2 amount/recipient → cmx | Codec-only K17; 170.788s total including checks, 11200 bytes; altered amount/recipient/hash/cmx/index rejected |
| Six-row SHA auction/native integration | 1654.925s MPC proof stage; normal C++ verification and six-row native link passed |

The hash-only Pallas gate has private field inputs; it does not itself bind them
to native note cells. A separate native binding circuit now copies the same
canonical point/amount words as the SHA circuit, then packs their actual byte
cells. The first execution configured unused SHA gates and took 546.586s total.
The byte-codec-only configuration removed those gates and passed in 170.788s,
with a command peak RSS of 3.49GB. These totals include mock checks, key generation
and proof generation. Six-row Poseidon timing with one reused native key remains
to be measured before selecting a commitment for integration.

Existing small-field variants such as [Plonky3's Goldilocks Poseidon2](https://github.com/Plonky3/Plonky3/blob/main/goldilocks/src/poseidon2.rs)
are another possible comparison. They require modular arithmetic on **both**
large-field sides, plus an appropriate published parameter/capacity/encoding
choice. Running a formula separately modulo BN254 and Pallas does not create
the same commitment. No speed or security level is assumed from the name, and
no third implementation is being added before the complete flow is measured.

```sh
cargo test --release --locked -p darkcall-bn254-emulation
cargo run --release --locked -p darkcall-bn254-emulation --bin poseidon_gate
cargo run --release --locked -p darkcall-output-binding --bin poseidon_binding
```

The SHA path remains available and is independently reverified after extracting
the shared native row-cell helper. Classical note commitment binding, rseed and
ciphertext limitations are unchanged for either additional proof.


## Six-row comparison

Both variants now passed the actual four-bid plan, every native row and the normal
C++ verifier. SHA MPC proof: 1654.925s. Poseidon2 MPC proof: 8.170s, with a 14656-byte
proof and about 601MB maximum RSS among three processes. Native Poseidon uses six
11200-byte proofs and one reused key: 72.662s key generation, 808.534s total
including proofs, verification and adversarial checks. Independent public native
verification took 117.439s with two Rayon threads. The initial native run used
default Rayon threads and encountered memory pressure (4.34GB peak RSS, 10.73GB
peak memory footprint), with concurrent compilation. These are observations,
not a controlled latency comparison or a production guarantee.

The offline four-input/FROST integration is testing Poseidon2. SHA remains
available. Auction conditions/receipts must authenticate the selected hash rule
and trusted circuit identities before real funding; there is no implicit rule
switch for an already funded auction.
