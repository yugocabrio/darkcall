# Design reviews incorporated on October 4

## Conditional settlement and custody

The current implementation remains REP3 plus 2-of-3 FROST. ZIP 374 pre-authorization
allows later v6 anchors/witnesses/proofs, while shielded signatures commit to all
shielded inputs and outputs. An unknown clearing price cannot simply be filled
into an already signed settlement transaction.

Participant N-of-N custody makes settlement abortable. Pre-authorized finite
outcome tables deserve later research, but narrowing spending authority requires
credible deletion of every signing-key copy and correct outcome release. Key
deletion is an additional assumption, not a publicly proved fact. A pre-signed
refund also does not automatically establish a deadline-enforced refund path.
Application-proof authorization would change Zcash consensus and requires
substantial security/protocol engineering. It is not an application-only feature.

The current committee's trust covers custody, viewing/settlement metadata and
availability; describing it as only the final step would hide material trust.
No alternative has been implemented or substituted for the current guarantees.

## Cross-field commitment performance

SHA-256 was chosen as a common byte-level hash between BN254/coNoir and
Pallas/Halo2. Four compression blocks per 224-byte row are costly in REP3's
proof-key construction, including garbled sparse normalization/table operations.
This is not the time of a normal CPU SHA operation or merely network latency.

An ARM AES-enabled build now completes the one-row MPC proof in 574.499 seconds,
with normal C++ verification. The successful configuration uses unchanged source
and lockfile, with `-C target-cpu=native --cfg aes_armv8`. CPU-native alone still
selected software AES. No further repeated build optimization is scheduled.

The human supplied a review proposing BN254 Poseidon2 inside coNoir and fully
constrained BN254 arithmetic inside the Pallas native circuit. This is now a
comparison task. It moves cost to a single prover, but limb ranges, carries,
quotients, canonical reductions and links to native cells must all be enforced.
The proposed 10-second-to-one-minute runtime is unmeasured. Retain the working
SHA proof as a reference until an alternative passes actual proofs, cross-language
vectors and adversarial arithmetic/linkage tests.

Sources: [ZIP 374](https://zips.z.cash/zip-0374), the pinned coNoir implementation,
and measurements in `evidence/2026-10-04-shared-mpc-native-row-gate.json`.
