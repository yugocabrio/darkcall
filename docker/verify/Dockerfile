# Clean-room public verifier for DarkCall settlement bundles.
#
# The build context is `git archive` of a committed revision (see
# scripts/docker_verify.sh): no .local state, keys, wallets or host binaries.
# Every tool is fetched at its pin and built from source inside Linux, then the
# container runs the same fixed verifiers as ./verify.sh --funded.
# Layers copy only what each build step needs, so later source edits do not
# rebuild the pinned toolchain.
FROM rust:1.93.0-trixie@sha256:bbde3ca426faeebab3eb58b8005d3d6da70607817a14bb92a55c94611d4d905f
ARG TARGETARCH
ARG JOBS=3
RUN apt-get update && apt-get install -y --no-install-recommends \
      cmake make clang libomp-dev python3 git curl ca-certificates procps nodejs npm \
    && rm -rf /var/lib/apt/lists/*
# Barretenberg's configure step runs `yarn --immutable` for its N-API headers
# (yarn@4.13.0 from its packageManager field, resolved through corepack).
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
RUN npm install -g corepack@0.34.0 && corepack enable
WORKDIR /src

# coNoir source pin: its committed CRS blobs are the verifier's CRS.
RUN mkdir -p .deps && git clone --filter=blob:none https://github.com/TaceoLabs/co-snarks.git .deps/co-snarks \
    && git -C .deps/co-snarks checkout --detach 467a2abbad940d40ed78f38d424cc21920988976

# Noir 1.0.0-rc.2 release binary, checked against GitHub's published digest.
RUN case "$TARGETARCH" in \
      arm64) a=aarch64-unknown-linux-gnu; d=ca17cae02570c0ca769d60c2dbb7601e0a7aa832f5d1fc77b50a7e33fb938c32 ;; \
      amd64) a=x86_64-unknown-linux-gnu; d=e07f914eeb8941476d41e14c6fb66bc25c96fc6c793603b1a30598fcd69f20c4 ;; \
      *) echo "unsupported $TARGETARCH"; exit 1 ;; esac \
    && curl --fail --location -o /tmp/noir.tgz "https://github.com/noir-lang/noir/releases/download/v1.0.0-rc.2/noir-$a.tar.gz" \
    && echo "$d  /tmp/noir.tgz" | sha256sum -c - \
    && mkdir .deps/noir-1.0.0-rc.2 && tar -xzf /tmp/noir.tgz -C .deps/noir-1.0.0-rc.2 && rm /tmp/noir.tgz \
    && .deps/noir-1.0.0-rc.2/nargo --version

# Barretenberg C++ verifier at its pin with exactly the two committed patches.
COPY patches /src/patches
RUN git clone --filter=blob:none --no-checkout https://github.com/AztecProtocol/aztec-packages.git .deps/barretenberg-src \
    && git -C .deps/barretenberg-src sparse-checkout set barretenberg \
    && git -C .deps/barretenberg-src checkout --detach 87aa5283d5b0cab6632ed49fdd25f6548d2ba91f \
    && git -C .deps/barretenberg-src apply ../../patches/barretenberg-apple-clang15.patch \
    && git -C .deps/barretenberg-src apply ../../patches/barretenberg-macos-ipc.patch \
    && CC=clang CXX=clang++ cmake -S .deps/barretenberg-src/barretenberg/cpp \
         -B .deps/barretenberg-src/barretenberg/cpp/build-darkcall \
         -G 'Unix Makefiles' -DCMAKE_BUILD_TYPE=Release -DBB_LITE=OFF -DAVM=OFF \
    && cmake --build .deps/barretenberg-src/barretenberg/cpp/build-darkcall --target bb -j "$JOBS" \
    && find .deps/barretenberg-src/barretenberg/cpp/build-darkcall -name '*.o' -delete

# The two Rust verifier helpers, from their unchanged lockfiles.
COPY Cargo.toml Cargo.lock /src/
COPY crates /src/crates
COPY vendor /src/vendor
COPY tools /src/tools
RUN cargo build --release --locked -p darkcall-native-gate -j "$JOBS" \
    && CARGO_TARGET_DIR=/src/.deps/nu7-compat-target \
       cargo build --release --locked --manifest-path tools/nu7-compat/Cargo.toml -j "$JOBS" \
    && rm -rf target/release/deps target/release/build target/release/incremental \
              .deps/nu7-compat-target/release/deps .deps/nu7-compat-target/release/build

# Circuits, scripts, public bundles and authorities: the rest of the revision.
COPY . /src
RUN .deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/settlement_poseidon4 \
    && .deps/noir-1.0.0-rc.2/nargo compile --program-dir circuits/delivery_flags4

CMD ["python3", "-B", "scripts/docker_verify_all.py"]
