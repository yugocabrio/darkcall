# Smallest output binding experiment

`darkcall-output-binding` uses the pinned Orchard NoteCommit gadget through its
`unstable-voting-circuits` API. It aims to prove that a private row's amount and
expanded recipient are the same inputs used by an actual native Sinsemilla note
commitment. The experimental row uses Pallas Poseidon P128Pow5T3, length 21.
This row commitment has not been connected to the BN254 auction circuit.

The prototype commits to a fixed domain, four u64 limbs each for the conditions,
accepted root and ticket, role, action index, both Pallas coordinates for g_d and
pk_d, amount and random field salt. Poseidon receives the same constrained cells
as NoteCommit; the amount cells are explicitly equated. Both row hash and native
extracted commitment are public inputs to the Halo2 proof.

The source creates a real Ironwood output with the native builder, decrypts it,
derives ZIP 2005's V3 rcm with the native library, and uses its opening as the
private witness. The circuit constrains the native commitment calculation, but
does not constrain the Blake2b rseed derivation. Its additional binding statement
therefore assumes classical commitment binding; it does not claim a separate
post-quantum opening guarantee. It also does not prove ciphertext correctness,
chain funding, finality or that the row was selected by the auction.

Run:

```sh
cargo run --release --locked -p darkcall-output-binding
```

The experiment verifies an actual Halo2 proof and rejects changes to row hash
or native commitment. Constraint checks reject a changed amount or recipient.
Execution evidence is recorded only after the experiment passes.

The October 4 run passed: K=12, 4,928 proof bytes, 9.53 seconds total after
compilation, and all four tampering checks rejected changes. See
`evidence/2026-10-04-native-output-binding-gate.json` for the limited statement.

Next, agree a commitment encoding computable by both the auction and binding
circuits. SHA-256 is supported by the pinned REP3 backend, but the stock Halo2
SHA-256 API exposes values rather than external cell links. Any adaptation must
explicitly constrain the message and digest cells; computing the same hash in
host code is insufficient. The Pallas prototype is a feasibility gate, not a
completed settlement proof or an adopted fallback to operator attestation.
