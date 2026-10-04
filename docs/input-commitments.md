# Input consistency gate

`circuits/bid_input` is proved by each client, who knows only its own bid and its
fresh shares. `circuits/auction4` is proved collaboratively by three REP3 nodes.
These are separate proofs with separate verification keys. Nodes must use the
key derived from the agreed circuit, never a key supplied by the client.

## Encoding

Poseidon2 is Noir rc.2's fixed-length BN254 sponge: width 4, rate 3, length times
2^64 in capacity, add/permutate complete blocks, then a final permutation after
the remaining fields. `crates/input-tool` matches the authoritative Noir vector.
Domains are big-endian ASCII integers; 32-byte IDs/digests are four big-endian
u64 limbs. They are never reduced into a single field.

- Context: H(`DarkCall/ctx/v1`, condition digest limbs, K, reserve, B, f).
- Bid leaf: H(`DarkCall/bid/v1`, context, ticket ID limbs, bid, random field salt).
- Party pair: H(`DarkCall/share/v1`, context, ticket ID limbs, party index,
  bid_a, bid_b, salt_a, salt_b).
- Accepted root: H(`DarkCall/accepted/v1`, context, capacity 4, accepted count,
  four active flags, four leaves).

For additive components x0+x1+x2, TACEO party i receives (xi, x(i+2)%3).
The client circuit checks the sums, range and leaf opening, and exposes three
pair hashes. A node recomputes its pair hash and compares the proof's exact
canonical public-input bytes with the expected ticket/context statement.
`darkcall-input-tool check` performs this comparison; the caller must also
cryptographically verify the client proof using the trusted key.

The auction circuit opens the same leaves in MPC, checks the accepted root, and
requires strictly sorted ticket IDs in a contiguous active prefix. Inactive
slots have zero IDs, bids, salts and leaves. It reveals price and sold count.

## What the gate establishes

The recorded synthetic run verifies four client proofs and twelve delivered
pairs, followed by a commitment-bound auction proof. A changed delivered share,
price or root is rejected. Noir tests reject removing an accepted ticket while
reusing the original root, changed bid openings and changed conditions.

The supplied root still needs an authoritative signed acceptance protocol.
An operator could otherwise publish a different root that omits an input; the
current circuit does not decide which root a user accepted. This gate also does
not prove funding, honest dealer setup, malicious MPC security or settlement.
Fresh random salts and shares are used even for the small synthetic bids.
