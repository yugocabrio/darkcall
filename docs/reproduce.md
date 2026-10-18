# Reproduce the public verification

## One command: clean-room Linux container

With Docker and at least 15 GB of free disk, from a checkout:

```sh
scripts/docker_verify.sh          # or: scripts/docker_verify.sh REVISION
```

This pipes `git archive` of a committed revision into `docker build`, so the
image sees committed files only: no `.local` state, keys, wallets or host
binaries. Inside Debian trixie (`rust:1.93.0-trixie`, pinned by digest) it
builds every tool from source at its pin:

- Noir rc.2, checked against GitHub's published release digest.
- The co-snarks CRS.
- Barretenberg `bb` with exactly the two committed patches.
- `darkcall-native-gate` and the NU7 SDK, from their lockfiles.

It then runs `scripts/docker_verify_all.py`. That script verifies every
committed funded bundle against its separately committed authority, then flips
one byte of each bundle's receipt proof and requires the fixed verifier to
reject it cryptographically.

On Oct 18 on the 16 GiB M1 (Docker Desktop, linux/arm64): `bb` 567 s, both
Rust helpers 450 s, then all three bundles verified in 5 min 23 s
(`browser-auction-nu7-v1` 112 s, `fresh-book-nu7-v1` 101 s,
`canonical-nu7-funded-v1` 110 s). Every corrupted receipt proof was refused.
This is a clean build environment on the same machine, not a second host.
A later rebuild run alongside heavy host swap exhausted the disk. Build only
with ample headroom and no concurrent proving, then remove the image with
`docker image rm` when you are done. See
[the record](../evidence/2026-10-18-clean-room-linux-verification.json).
amd64 is supported by the Dockerfile but was not run.

The first container builds also found two portability defects that a same-host
checkout hid. Both were fixed without weakening any check:

- `.gitignore`'s `**/*witness*` rule had excluded the vendored source
  `vendor/halo2_gadgets/src/ecc/chip/witness_point.rs`, so a fresh clone could
  not compile.
- The exact Barretenberg patch check compared `git diff` text, which
  abbreviates index hashes differently in larger clones. The check now pins
  `core.abbrev=7`.

Barretenberg's configure step also needs Node and Yarn for its N-API module;
the image provides them through pinned corepack.

## Manual setup

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

## Full local product

Creating auctions, clearing them and settling them on the local NU7 regtest
needs every pinned tool. After the public-replay tools above, also build the
following:

```sh
cargo build --release --locked --manifest-path .deps/co-snarks/Cargo.toml --bin plaindriver -j 2
CARGO_TARGET_DIR="$PWD/.deps/co-snarks/target-aes-native" RUSTFLAGS='-C target-cpu=native --cfg aes_armv8' \
  cargo build --release --locked --manifest-path .deps/co-snarks/Cargo.toml --bin co-noir -j 2
cargo build --release --locked -p darkcall-native-gate -p darkcall-input-tool \
  -p darkcall-output-binding -p darkcall-wallet-bridge --bins --examples -j 4
.deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/bid_input
```

`darkcall-output-binding` provides `poseidon_batch`, which the settlement
re-verifies before FROST signing. A settlement attempt on Oct 18 without it
stopped at that check, before any reservation or signature, and was resumed from
the exact prepared transaction once the binary was built. `settle_fresh_book.py`
and the server now refuse to start settlement when a tool is missing.

The chain side needs Docker, the pinned Zebra 7.0.0-rc.0 archive and one funded
local test bank ([regtest.md](regtest.md)):

```sh
python3 scripts/regtest_nu7_node.py prepare
python3 scripts/regtest_nu7_node.py start
python3 scripts/regtest_nu7_node.py mine 2              # fresh chain only
python3 scripts/regtest_nu7_funding.py --name nu7-bank-20261005   # fresh chain only
./demo.sh
```

`settle_fresh_book.py` uses `.local/nu7-bank-20261005` by default (`--bank`
selects another). It rescans the bank at the current tip before every
settlement. Each settlement spends 4 × 15100 + 25000 zatoshi of regtest funds
and mines 4 controlled blocks. A CLI settlement of a certified browser book is:

```sh
python3 -B scripts/settle_fresh_book.py .local/browser-rehearsal-SESSION/AUCTION_ID \
  --name settle-new --expect PRICE,SOLD --progress .local/settle-new-progress.jsonl
```

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
