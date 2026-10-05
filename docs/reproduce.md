# Reproduce the public verification

The canonical four-bid funded auction is already proved. Public replay needs
the committed source, the 154KB public bundle and a separate caller-trusted
authority. It needs no private fixture, live node, deposit or proof generation.
The complete funded-regtest regeneration is a different, heavier procedure in
[regtest.md](regtest.md). Historical notes are already spent; never rebuild or
rebroadcast their transaction as a new auction.

These commands describe the pinned Apple Silicon toolchain that passed locally.
A second machine, Linux source build and parallel measurements remain unverified.
Keep the exact source/patch checks when reproducing. Do not bypass a refusal to
make a new platform appear supported.

## Source and public-only browser

Use a trusted project revision or the locally prepared review kit. The kit is
an archive of committed files, not the developer's working directory. Check its
SHA256 and expected Git revision against an independently obtained record; its
own manifest is not a signature. A trusted checkout can audit without extracting:

```sh
python3 -B scripts/review_kit.py check ARCHIVE.tar.gz --expected-commit FULL_COMMIT
```

Extract only an audited archive into a new directory, then enter `source/`.
Python 3.10+ and a browser are enough to start the public-only view:

```sh
./demo.sh --public-only
```

No keys are shipped, so own-result opening is unavailable there. Actual tamper
checks and full settlement verification require the pinned tools below. This is
not a simulated verifier. Missing prerequisites cause a refusal.

## Pinned dependencies for public replay

Choose the final source directory **before building**. The Rust helpers use
`CARGO_MANIFEST_DIR` for trusted circuit/scripts and ignored snapshot paths.
Copying their binaries to a different directory is not supported. Rebuild there.
No existing `.local` fixture or wallet should be copied for public verification.

Prerequisites: Rust 1.93.0, Git, Python 3.10+, CMake, a C++20 toolchain, make,
and curl/tar for the pinned compiler archive. Keep substantial disk headroom for
Rust/C++ compilation. Run one build/heavy verifier at a time on the 16GiB Mac.
Download/build dependencies under ignored `.deps`:

```sh
mkdir -p .deps
git clone https://github.com/TaceoLabs/co-snarks.git .deps/co-snarks
git -C .deps/co-snarks checkout --detach 467a2abbad940d40ed78f38d424cc21920988976
git clone --filter=blob:none --no-checkout https://github.com/AztecProtocol/aztec-packages.git .deps/barretenberg-src
git -C .deps/barretenberg-src checkout --detach 87aa5283d5b0cab6632ed49fdd25f6548d2ba91f
```

The co-snarks commit carries `bn254_g1.dat`, `bn254_g2.dat` and
`grumpkin_g1.dat` under `co-noir/co-noir-common/src/crs`. No giant replacement
CRS download is needed for this fixed replay. The preflight compares them with
the pinned Git blobs. Preserve the upstream notices in those source checkouts.

On Apple Silicon, download and checksum the compiler **before** extraction:

```sh
curl --fail --location --output .deps/noir-aarch64-apple-darwin.tar.gz \
  https://github.com/noir-lang/noir/releases/download/v1.0.0-rc.2/noir-aarch64-apple-darwin.tar.gz
python3 -c "import hashlib,pathlib; p=pathlib.Path('.deps/noir-aarch64-apple-darwin.tar.gz'); assert hashlib.sha256(p.read_bytes()).hexdigest()=='e44a46a3c35e89b064948c054562da36b8cc58bfb31623ff27e5440c3a0bb2bc'"
mkdir .deps/noir-1.0.0-rc.2
tar -xzf .deps/noir-aarch64-apple-darwin.tar.gz -C .deps/noir-1.0.0-rc.2
.deps/noir-1.0.0-rc.2/nargo --version
.deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/settlement_poseidon4
.deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/delivery_flags4
```

Build the **exact patched C++ source**, using file-based verification. The two
patches address Apple Clang deduction and unavailable macOS IPC symbols; neither
changes the proof statement. The verifier currently expects both exact patches,
including on an untested Linux build. The macOS IPC changes are inside `__APPLE__`.

```sh
git -C .deps/barretenberg-src apply ../../patches/barretenberg-apple-clang15.patch
git -C .deps/barretenberg-src apply ../../patches/barretenberg-macos-ipc.patch
cmake -S .deps/barretenberg-src/barretenberg/cpp \
  -B .deps/barretenberg-src/barretenberg/cpp/build-darkcall \
  -G 'Unix Makefiles' -DCMAKE_BUILD_TYPE=Release -DBB_LITE=OFF -DAVM=OFF
cmake --build .deps/barretenberg-src/barretenberg/cpp/build-darkcall --target bb -j 2
```

The current public path uses these two native helper binaries, built with their
unchanged lockfiles from the final source directory:

```sh
cargo build --release --locked -p darkcall-native-gate -j 2
CARGO_TARGET_DIR="$PWD/.deps/nu7-compat-target" \
  cargo build --release --locked --manifest-path tools/nu7-compat/Cargo.toml -j 2
python3 -B scripts/reproduction_check.py
./verify.sh --funded demo/artifacts/canonical-nu7-funded-v1 \
  --authority demo/authority/canonical-nu7-funded-v1.json
```

On the original Mac, the Homebrew `protoc` binary has an unavailable Abseil
library. The recorded SDK build excludes that broken binary from `PATH`; pinned
upstream then uses its checked-in generated Rust. No dependency source is patched.
Fix the tool or supply a PATH containing working Cargo, compiler and system tools
but excluding the broken `protoc`; do not copy a host-specific PATH blindly.

The preflight inventories exact pins, patched source, CRS blobs, binary/circuit
digests and public artifact/context links. It invokes no keys, RPC or verifier.
**The subsequent fixed proof/native replay is the validation step**, not preflight.
Reported local replay was 112.431s on a shared M1; timings on other hosts must be
measured rather than inferred.

## Recipient demo and regeneration

The development demo has ignored fixture own-FVK exports and can open all four
results. A source-only review kit cannot reproduce those openings without a new
fixture. Do not distribute identities to turn the public-only page into an
owner-client demo. `docs/demo.md` has a fresh synthetic regeneration procedure;
`docs/regtest.md` has actual funded DKG/admission commands and their limitations.
coNoir, plaindriver, input-tool, additional circuits and an isolated node are
needed for regeneration; they are intentionally not required for public replay.

## Local review-kit creation

Commit the intended source first, then:

```sh
python3 -B scripts/review_kit.py create --revision HEAD --output .local/review-kit.tar.gz
python3 -B scripts/review_kit.py check .local/review-kit.tar.gz --expected-commit FULL_COMMIT
python3 -B scripts/check_review_kit.py .local/review-kit.tar.gz --expected-commit FULL_COMMIT
```

The deterministic archive contains committed source/public artifacts and a short
review entry point, with file digests, lock digests and the exact source revision.
Links/submodules, ignored/runtime/private paths, common credential markers,
oversized members and overwrites are refused. This is a bounded packaging check,
not an exhaustive secret audit or producer authentication. No dependency binaries
or Git configuration are exported. Nothing is uploaded or published by the command.
The extracted-browser check creates only a new ignored public snapshot and stops
its own temporary server. It records same-host API/static/missing-key/missing-tool
behavior; it does not claim cryptographic replay or another-machine reproduction.
That extraction smoke requires Python3.12+ for the standard-library data filter;
the read-only archive audit and public-only browser need Python3.10+.
