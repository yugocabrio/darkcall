# Cryptographic toolchain pins

| Component | Pin | Evidence |
| --- | --- | --- |
| co-snarks | 467a2abbad940d40ed78f38d424cc21920988976 | Upstream workspace declares Rust 1.90 and Noir rc.2 |
| Noir / Nargo | v1.0.0-rc.2 | coNoir compatibility README |
| Barretenberg | v5.0.0-nightly.20260522 | coNoir compatibility README; not installed yet; global binary is an older nightly |
| Local Rust | 1.93.0 | Actual rustc output on October 4, 2026 |

Apple Silicon Noir release archive:
https://github.com/noir-lang/noir/releases/download/v1.0.0-rc.2/noir-aarch64-apple-darwin.tar.gz

SHA256 (GitHub release asset digest, checked before extraction):
e44a46a3c35e89b064948c054562da36b8cc58bfb31623ff27e5440c3a0bb2bc

Downloaded tools and source live under ignored .deps/. Existing global Nargo
beta.20 has not been replaced.

Upstream source: https://github.com/TaceoLabs/co-snarks/tree/467a2abbad940d40ed78f38d424cc21920988976

The upstream co-noir crate is MIT OR Apache-2.0. Dependency license review is
still required before the complete distribution.

## Initial commands

```sh
cargo test --workspace
.deps/noir-1.0.0-rc.2/nargo test --program-dir circuits/auction4
.deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/auction4
cargo build --release --locked --manifest-path .deps/co-snarks/Cargo.toml --bin co-noir -j 2
```

Initial circuit: calculation feasibility only. It does not authenticate funding
tickets, bind bids to accepted commitments, or prove native settlement.

## Collaborative calculation gate

After building the pinned coNoir binary:

```sh
python3 -B scripts/run_mpc_gate.py
```

The script creates four separate synthetic bidder input files, splits and merges
REP3 shares, runs three localhost processes, generates the witness and ZK-enabled
proof, verifies it using the standalone coNoir verifier and rejects an altered
public clearing price. All private inputs, shares, TLS fixture keys and witnesses
remain under ignored .local/ or .deps/. Published evidence is only a summary.
The public TLS test keys from upstream are for localhost testing only.

At this upstream pin, the binary verifier treats a public-input filename with an
extension as a format mismatch. The runner uses extensionless public-input files.
Matching external Barretenberg verification is still an unfinished gate.

Nargo rejects prerelease versions in the package compiler_version field.
The rc.2 pin is enforced by the selected binary path, not that field. Initial
manifest validation failed with that field and was corrected before the passing
test/compile runs. The Barretenberg release API for this tag returned 404;
source-based installation or an official matching artifact still needs evaluation.
