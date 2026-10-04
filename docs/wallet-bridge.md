# Wallet bridge scope

`darkcall-wallet-bridge` currently identifies a test chain through read-only TLS
lightwalletd calls. It rejects a non-test chain before further queries, verifies
the explicitly selected genesis, cross-checks the tip's compact-block/tree hashes,
and compares the reported consensus branch against pinned network rules. It
reports Ironwood tree metadata without claiming wallet funding or consensus
validation. A lightwalletd provider remains trusted for these observations.

```sh
cargo build --release --locked -p darkcall-wallet-bridge
# Standard Zcash testnet. Never replace this with a mainnet endpoint.
target/release/darkcall-wallet-bridge probe-testnet https://testnet.zec.rocks:443
# Explicit alternative scope; this is not standard testnet.
target/release/darkcall-wallet-bridge probe-isolated-regtest \
  https://lwd.157.245.208.35.sslip.io:443
```

The network distinction matters because `LightdInfo.chain_name = test` is also
used by isolated regtest deployments. The genesis pins come from
[Zcash chain parameters](https://github.com/zcash/zcash/blob/master/src/chainparams.cpp).
The isolated endpoint and its scope are described by its
[operator's integration documentation](https://github.com/valargroup/ironwood-testnet/blob/main/docs/integrating.md).
The protocol client is the generated official SDK interface pinned in Cargo.lock;
API definitions are in the [lightwallet protocol](https://github.com/zcash/lightwallet-protocol).
Endpoint availability and actual activation must be measured, never inferred from
these documents alone. TLS certificates remain verified; no plaintext fallback.

## Durable private funding state

The `journal::Journal` library wraps the already tested funding state machine.
It records network genesis, conditions hash, commitment rule and terms in a
private journal header. Events contain note IDs, actual deposit values, blocks,
tickets and transaction reservations. They contain no maximum bids or keys.
These records still expose wallet metadata and must stay private and ignored.

Every successful transition is appended and fsynced before its in-memory state
is updated. An exclusive OS file lock prevents two local writers. Replay checks
context, sequence, the exact state transitions and terminal newline. Partial or
malformed writes fail closed and need explicit recovery; they are not silently
skipped. A failed append poisons the handle. A successfully written reservation
survives restart, preventing a competing settlement/cancellation reservation.
Private directory/file permissions, regular-file shape, single-link metadata and
no-follow opening guard against accidental exposure or symlink reuse.

This journal is local crash recovery. It is not an authenticated funding proof,
operator receipt, protection against a malicious administrator editing storage,
or a distributed agreement protocol. The caller must validate canonical-chain
observations, funding confirmations, client proofs and delivered shares before
appending the corresponding events. Wallet scanning, Merkle witnesses, finality,
reorg recovery after preparation and signed acceptance are still separate work.
