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
protection against a malicious administrator editing storage,
or a distributed agreement protocol. The caller must validate canonical-chain
observations, funding confirmations, client proofs and delivered shares before
appending the corresponding events. Wallet scanning, Merkle witnesses, finality,
reorg recovery after preparation remain separate work.

## Native scanning and authenticated claims

The new `scanner::Scanner` consumes explicitly selected test-chain compact
blocks from a checked initial tree before the scanning birthday. It decrypts
native V3 Ironwood ciphertexts with the escrow viewing key, appends every output
commitment, updates owned-note witnesses and detects owned nullifiers. Malformed
blocks, noncontiguous hashes and inconsistent tree sizes fail without advancing
the cursor. Check the resulting anchor against the provider's tree snapshot.
This is an in-memory library: retained-block recovery, durable cursor/witnesses,
RPC synchronization, finality and reorg/spent-funding reconciliation are not wired.
The provider and initial checkpoint remain trusted; it does not validate consensus.

`claim` authenticates the client instructions for an actual full Ironwood output.
The encrypted 512-byte deposit memo contains network genesis, conditions, ticket,
a fresh application Ed25519 key and a fresh private nonce. A strict client
signature covers the memo digest, actual cmx, bid commitment, three delivered-pair
commitments and refund receiver commitment. Another person knowing a public cmx
cannot use it to substitute their instructions. The verifier decrypts the actual
full output and checks exact context, amount and version. The caller must first
check that transaction/output corresponds to canonical confirmed unspent funding,
and independently verify client ZK proofs and delivered-share pairs. A valid
claim alone does not admit a bid or prove funding to the public.

## Signed application acceptance

`receipt` uses three distinct trusted application identities, separate from
native FROST custody keys. Each operator checks its accepted registration against
the verified claim and fsyncs a payload lock before signing. Restart can reissue
the identical signature; it cannot endorse different instructions for that ticket.
Three matching strict signatures form an accepted receipt. Public payloads expose
opaque tickets/commitments and an intent digest, not funding cmx, nonce, amount,
address openings or viewing keys. Configure identities from trusted auction
setup; a receipt must never supply its own authority.

These signatures attest operator acceptance. They do not replace client proofs,
prove canonical funding, certify the complete deadline book or authenticate the
actual MPC/native bundle yet. All three signatures are required; this shares the
REP3 computation's availability requirement. Final-root certification and exact
public-statement linkage are the next implementation step.
