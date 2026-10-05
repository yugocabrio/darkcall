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
