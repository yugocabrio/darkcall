# Isolated NU7 native compatibility gate

This standalone Cargo workspace preserves the successful NU6.3 application's
manifest, lockfile, binaries, wallets and isolated chain. It is an experiment
toward supporting public testnet, not a migration of the auction implementation.

The native SDK is pinned to librustzcash
`9c5705f56517c040c50f96d29f1ff15a9383f3e0`: protocol 0.11.0-pre.0, primitives
0.31.0-pre.0, PCZT 0.10.0-pre.0 and backend 0.25.0-pre.0. Its testnet rules
activate NU7 (`77190ad9`) at height 4,465,026. Orchard 0.16.0 uses the new
ff/group/Pasta/Halo2 family. The existing pinned RedPallas FROST implementation
uses the older Rust types; canonical scalar/key/signature bytes cross that type
boundary, and native verification checks the resulting signatures.

Run from the repository root:

```sh
CARGO_TARGET_DIR=.deps/nu7-compat-target CARGO_BUILD_JOBS=2 cargo build --release --locked --manifest-path tools/nu7-compat/Cargo.toml
CARGO_TARGET_DIR=.deps/nu7-compat-target cargo test --release --locked --manifest-path tools/nu7-compat/Cargo.toml
.deps/nu7-compat-target/release/darkcall-nu7-compat probe-testnet https://testnet.zec.rocks:443
.deps/nu7-compat-target/release/darkcall-nu7-compat synthetic-native
.deps/nu7-compat-target/release/darkcall-nu7-compat note-interop
.deps/nu7-compat-target/release/darkcall-nu7-compat verify-old-native .local/regtest-auction4-public-20261005/regtest-transaction.bin
```

On this Mac, the installed Homebrew `protoc` cannot load its old Abseil library.
The upstream SDK intentionally supports building its checked-in generated Rust
types when `protoc` is absent. The recorded build used
`PATH=/Users/yugo/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin` before Cargo so that it
took that path. No SDK files or system installations were modified. On another
host, use its working toolchain path or a working `protoc`; do not copy the local
absolute PATH blindly.

- The HTTPS probe has no keys, funding or broadcast API. It rejects other
  genesis hashes, unknown/mismatched branches and inconsistent tip snapshots.
  It checks the prior frontier, tip commitments and reported final frontier.
  The provider remains trusted: this is not consensus/SPV verification.
- The synthetic gate fabricates a note and Merkle anchor. **Never broadcast its
  transaction.** It tests native v6 NU7 PCZT construction, private-field checks,
  consensus proof, 2-of-3 rerandomized FROST, full public proof/signature replay
  and two exact recipient decryptions. It does not run an auction/application
  binding proof. Setup is a local trusted dealer, not DKG.
- The synthetic builder explicitly allows 240 blocks before expiry. This is
  compatibility coverage, not a finalized public-chain transaction freshness or
  rescan policy. The existing auction's unchanged-tip regtest driver is not
  appropriate for a live chain.
- `note-interop` reconstructs seven new-SDK V3 note openings in the exact working
  Orchard fork through canonical recipient/rho/rseed/FVK bytes. It compares cmx,
  nullifier and psi/rcm_v3 witness bytes, alternating internal/external addresses
  at boundary amounts, and checks changed amounts change commitments. Both pins
  expose their witness accessors with upstream `unstable-voting-circuits`; no
  upstream source patches or Rust field/group type sharing are used. It produces
  no proof and does not migrate the actual auction binding stack. Evidence:
  `evidence/2026-10-05-nu7-note-interop.json`; the separate updated inventory records
  295 packages and their declared licenses. This includes an old registry RedDSA
  0.5.2 transitive for note semantics, alongside the independently pinned 0.5.1
  Git FROST fork; it does not change the working root lockfile.
- The old-transaction command checks native proof/signatures and rejects treating
  the NU6.3 transaction as NU7. It does not check application proofs or inclusion.

Keep the separate lockfile. Actual measurements and any limitations belong in
the repository's evidence directory after running these gates.

## Four-input/six-output auction adapter

`python3 scripts/run_nu7_auction.py BOOK_RUN --name NEW_NAME --prepare-only`
verifies actual blinded v2 delivery with the working fixed C++ verifier, creates
an unsigned new-SDK PCZT with four fabricated input notes and six outputs, and
checks all six canonical V3 openings in the working SDK. Both price25/sold3 and
price0/sold3 pass. Zero-price preparation uses the separately masked winning
bits. No maximum-bid list is opened by this adapter; the books are fabricated
client fixtures and remain on one host.

This new mode deliberately stores its synthetic PCZT, retained master key and
recipient FVKs in ignored 0600 files under a 0700 directory. It does **not**
change the one-output export mode's key-free format. Never copy these prepared
private files into public artifacts. This is not the existing funded DKG flow.

Without `--prepare-only`, the adapter is designed to generate six shared18 row
proofs, run actual committed REP3 settlement and the fixed C++ verifier, and have
the new SDK independently verify those public proofs against its in-memory PCZT
effects before 2-of-3 signing. Both SDKs exchange bytes only, not field/group
types; the root lock and historical working33 verifier are unchanged. A public
replay command is provided by `scripts/replay_nu7_auction.py`.

**The complete new adapter and its public replay are not yet demonstrated.**
The full run was refused before launching by the unchanged 10 GiB initial-disk
guard (6.94 GiB free, 5.95 GiB used swap). Preserve the prepared checkpoints and
run the full gate only after capacity changes. Seven actual unsigned pre-sign
attack cases pass, including refusing absent application proofs, changed
amount/receiver/effects/index, unknown fields and trailing PCZT bytes. The
upstream decoder accepted trailing data, so this persisted-request adapter
requires exact canonical round-trip bytes. See
`evidence/2026-10-05-nu7-six-output-preparation.json`.

Public-chain funding, DKG migration, authenticated bidder/receiver receipts,
live-chain freshness and finality remain separate unfinished work. No broadcast
operation is added here.

## Encrypted ticket memo component

`receipt-transport DIRECTORY` reads four private canonical ticket memos and a
bounded synthetic amount request, then builds/proves/FROST-signs a fabricated
one-input/four-output native transfer. Its output is never broadcast. This is
separate from the six-output auction and performs no auction row binding proof.
`receipt-transport-open RECIPIENT_DIRECTORY INDEX OUT_MEMO` verifies the native
transaction against that recipient's expected identity and decrypts with its
own viewing key only. It does not verify the ticket proof: the client must first
check the compiled fixed C++ flags verifier, then open the memo with its own
expected book/ticket. `scripts/check_receipt_transport.py` checks that order and
the actual independent recipient processes at zero/positive prices.152 request
substitutions,24 cross-recipient decryptions and encrypted-memo native/AEAD
mutations reject. No secret keys/pads or memos are public evidence. See
`evidence/2026-10-05-encrypted-ticket-memos.json`.

The recipient wrapper `scripts/open_ticket_memo.py EXPECTED_BOOK PUBLIC_FLAGS_PROOF
OWN_RECEIVER_DIRECTORY TICKET_INDEX --name NEW_CHECKPOINT` performs fixed C++
verification, native authentication/decryption and expected ticket opening in
that order. It requires only the three real proof files and the recipient's own
identity. It never consumes operator pads or another recipient's viewing key.
It remains a local synthetic client, not advancing-chain wallet delivery.

## Actual new-SDK output bytes and existing binding circuit (unfinished proof)

```sh
.deps/nu7-compat-target/release/darkcall-nu7-compat synthetic-native-export NEW_EXPORT_NAME
.deps/nu7-compat-target/release/darkcall-nu7-compat verify-export .local/NEW_EXPORT_NAME
# Heavy old-layout proof: at least 16 GiB initial free disk on the current host.
# Keep resource guards; both previous runs stopped below 4 GiB, not proof failure.
python3 scripts/run_nu7_output_binding.py .local/NEW_EXPORT_NAME --name NEW_BINDING_NAME
# Requires an actually completed native-binding-proof first:
python3 scripts/replay_nu7_bound_output.py .local/NEW_EXPORT_NAME .local/NEW_BINDING_NAME --name NEW_PUBLIC_REPLAY_NAME
```

Exports use new 0700 directories and 0600 fsynced files under ignored `.local`.
Only native note opening bytes are private; no keys or PCZT are exported. The old
SDK reconstructs a V3 note from canonical bytes for the existing trusted row
circuit. It cannot parse NU7 consensus. Public replay freezes four public files:
the new SDK verifies the actual native proof/signatures/commitment mapping, and
the working SDK independently verifies the row proof under its fixed circuit.
It does not copy note openings, wallets or private row salts. This is one synthetic
output with a synthetic context, not certified auction admission or public funding.

Native export/signatures/decryptions and preflight byte/tampering checks passed.
**The new-output binding proof and complete public replay remain unfinished:**
resource guards stopped key/proof generation at 210.430s (two threads) and 341.850s
(one thread), with sampled free disk below 4 GiB. No proof bytes were produced;
the source exports and actual funded chain/journals were retained. See
`evidence/2026-10-05-nu7-binding-resource-stop.json` and
`evidence/2026-10-05-nu7-byte-input-checks.json`. No third identical heavy attempt
on the same constrained storage, no relaxed binding or branch validation.


## Explicit experimental shared-column layout

The separately identified `experimental-shared18-native-row-v1` circuit reuses
18 advice columns across disjoint native/encoding/BN254 regions, instead of 33.
It uses exactly the same gates, lookup tables, synthesis, canonical byte packing
and cross-region copy constraints. Existing funded statements and their original
trusted VK remain on the working33 circuit. A configuration fingerprint regression
checks the original layout has not changed. Both variants use K17 and degree9.
The 8x extended domain remains; column count alone is not a memory benchmark.

```sh
python3 scripts/run_nu7_output_binding.py .local/NEW_EXPORT_NAME --name SHARED_MOCK_NAME --shared-mock
# At least 10 GiB initial disk, all normal runtime guards, one heavy proof only:
python3 scripts/run_nu7_output_binding.py .local/NEW_EXPORT_NAME --name SHARED_PROOF_NAME --shared-proof
# Only after an actual shared proof exists; caller explicitly chooses fixed VK:
python3 scripts/replay_nu7_bound_output.py .local/NEW_EXPORT_NAME .local/SHARED_PROOF_NAME --name SHARED_REPLAY_NAME --shared-layout
```

Shared proof files have distinct names and a strict public layout wrapper. The
public native SDK compares the nested statement with actual serialized actions;
the explicit shared verifier chooses a hardcoded trusted circuit. A proof cannot
supply an arbitrary circuit/key. MockProver passed the correct new-SDK output and
rejected altered amount, recipient point, digest, cmx and index in 57.485s; this
is constraint testing, not a SNARK or reduced PK/proving memory demonstration.
The actual key/proof run is recorded in STATUS.md and separate evidence.


The explicit shared18 actual proof now passes: 9,504 bytes, 115.076s key,
104.009s proof/initial verify, 221.541s total. Public-only new-native/row replay
passes in 107.893s; hash/cmx/index/proof and native txid/action mapping changes
reject. This is one synthetic-context output, not a funded NU7 auction or full
SDK migration. Original working33 new-output proof remains resource-limited.
Shared host load changed: these observations establish completion, not controlled
memory/latency improvement. Used swap peaked at a sampled 12.31GB; min free disk
was 4.96GB. See `evidence/2026-10-05-shared-binding-layout.json`.
