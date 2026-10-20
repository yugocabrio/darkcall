# Hosted DarkCall demo: the landing at / and the live demo at /demo/settled and
# /demo/run, served by scripts/serve_demo.py --public-origin behind the TLS
# proxy in compose.yaml.
#
# The build context is `git archive` of a committed revision (see
# scripts/hosted_demo_build.sh): no .local state, keys or wallets. The recorded
# run and the testnet bank are mounted read-write at /src/.local at run time
# (scripts/hosted_local_set.py lists them); they never enter an image layer.
# Every tool is fetched at the same pins as docker/verify and built here.
FROM rust:1.93.0-trixie@sha256:bbde3ca426faeebab3eb58b8005d3d6da70607817a14bb92a55c94611d4d905f
ARG TARGETARCH
ARG JOBS=4
RUN apt-get update && apt-get install -y --no-install-recommends \
      cmake make clang libomp-dev python3 git curl ca-certificates procps nodejs npm \
    && rm -rf /var/lib/apt/lists/*
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
RUN npm install -g corepack@0.34.0 && corepack enable
WORKDIR /src

# coNoir at its pin: the REP3 prover (AES backend as on the measured host) and
# plaindriver for client proofs.
RUN mkdir -p .deps && git clone --filter=blob:none https://github.com/TaceoLabs/co-snarks.git .deps/co-snarks \
    && git -C .deps/co-snarks checkout --detach 467a2abbad940d40ed78f38d424cc21920988976 \
    && case "$TARGETARCH" in arm64) flags='-C target-cpu=native --cfg aes_armv8' ;; *) flags='-C target-cpu=native' ;; esac \
    && CARGO_TARGET_DIR=/src/.deps/co-snarks/target-aes-native RUSTFLAGS="$flags" \
       cargo build --release --locked --manifest-path .deps/co-snarks/Cargo.toml --bin co-noir -j "$JOBS" \
    && cargo build --release --locked --manifest-path .deps/co-snarks/Cargo.toml --bin plaindriver -j "$JOBS" \
    && rm -rf .deps/co-snarks/target-aes-native/release/deps .deps/co-snarks/target-aes-native/release/build \
              .deps/co-snarks/target/release/deps .deps/co-snarks/target/release/build

# Noir 1.0.0-rc.2 release binary, checked against GitHub's published digest.
RUN case "$TARGETARCH" in \
      arm64) a=aarch64-unknown-linux-gnu; d=ca17cae02570c0ca769d60c2dbb7601e0a7aa832f5d1fc77b50a7e33fb938c32 ;; \
      amd64) a=x86_64-unknown-linux-gnu; d=e07f914eeb8941476d41e14c6fb66bc25c96fc6c793603b1a30598fcd69f20c4 ;; \
      *) echo "unsupported $TARGETARCH"; exit 1 ;; esac \
    && curl --fail --location -o /tmp/noir.tgz "https://github.com/noir-lang/noir/releases/download/v1.0.0-rc.2/noir-$a.tar.gz" \
    && echo "$d  /tmp/noir.tgz" | sha256sum -c - \
    && mkdir .deps/noir-1.0.0-rc.2 && tar -xzf /tmp/noir.tgz -C .deps/noir-1.0.0-rc.2 && rm /tmp/noir.tgz

# Barretenberg at its pin with the two checked source patches.
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

# The workspace binaries and examples the drivers call, and the NU7 SDK.
COPY Cargo.toml Cargo.lock /src/
COPY crates /src/crates
COPY vendor /src/vendor
COPY tools /src/tools
RUN cargo build --release --locked --workspace --bins --examples -j "$JOBS" \
    && CARGO_TARGET_DIR=/src/.deps/nu7-compat-target \
       cargo build --release --locked --manifest-path tools/nu7-compat/Cargo.toml -j "$JOBS" \
    && rm -rf target/release/deps target/release/build target/release/incremental \
              .deps/nu7-compat-target/release/deps .deps/nu7-compat-target/release/build

# Circuits, scripts, the UI, public bundles and authorities.
COPY . /src
# The drivers recompile circuits at run time, so circuits/ belongs to the
# unprivileged user too.
RUN for d in circuits/*/; do \
      if [ -f "$d/src/main.nr" ]; then .deps/noir-1.0.0-rc.2/nargo compile --program-dir "$d" || exit 1; fi; \
    done \
    && useradd --uid 10001 --home-dir /src --no-create-home darkcall \
    && mkdir -p /src/.local && chown darkcall /src/.local && chown -R darkcall /src/circuits
USER darkcall
# External tools may cache under $HOME; /src stays read-only to this user.
ENV HOME=/tmp/darkcall

ENV DARKCALL_RUN=settle-aa36c45a6a-settlement
CMD python3 -B scripts/serve_demo.py --canonical --funded --run "$DARKCALL_RUN" \
      --bundle demo/artifacts/testnet-auction-nu7-v2 --authority demo/authority/testnet-auction-nu7-v2.json \
      --recorded-note "Created, funded, cleared and settled in this UI on public Zcash testnet on Oct 19" \
      --chain testnet --public-origin "$DARKCALL_ORIGIN"
