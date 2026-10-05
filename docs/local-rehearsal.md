# Seller and bidder entry rehearsal

The browser now has a working entry loop before the recorded settlement:

**Create → share locally → seal a demo bid → signed acceptance → certify the
closed book → fresh REP3 clearing → own verified result → tamper refusal.**

The entry and recorded settlement are **different auctions**. New inputs have
fresh conditions, tickets, randomness and admission keys. They do not become
part of transaction47019e02…a211938 or select its winning ticket. They now produce
their own fresh MPC clearing for the exact certified root, followed by local
opening of that ticket's refund entitlement and winning status. They do not
produce a native settlement. The optional recorded settlement remains separate.
Existing funded transaction/proofs and verifier remain intact.

## Start

Build the [pinned tools](toolchains.md), including the plaindriver client prover,
fixed C++ Barretenberg verifier and `circuits/bid_input` artifact. Then:

```sh
cargo build --locked --release -p darkcall-input-tool
cargo build --locked --release -p darkcall-wallet-bridge --example rehearsal_admission
./demo.sh
```

Open `http://127.0.0.1:8877`, choose **Create an auction**, set units1–3, reserve,
fixed budget and a local close time30seconds–24hours ahead. Default cutoff is
five minutes ahead. Share the generated localhost link in another browser on
this PC. It neither publishes a hosted market nor allows remote operator access.
The rehearsal is available on the development checkout with ignored fixture
identities and built input tools. `--public-only` disables it; fresh public-only
checkouts still verify the recorded bundle and reject tampering without keys.

Enter a **demo** maximum≤budget and press **Seal bid**. Copy the public signed
acceptance if needed. Four accepted tickets are required for this fixed prototype
book. After its actual wall-clock deadline, **Certify closed book** verifies the
four receipt quorums and issues three final-book signatures. It does not compute
the winners. **Clear this auction** uses those exact commitments/root and the
previously accepted shares in the existing `delivery_flags4/v2` REP3 circuit,
with fresh masks/blinders. No bid-opening files are read during this connection.
A fixed C++ verifier checks the new proof; a separate certificate gate compares
its exact public book fields with the signed book. **Open this ticket's result**
opens the ticket accepted in this page session, after both gates. The refund is
an entitlement, not a sent payment. **Tamper with clearing proof** runs a real
negative verification without reading ticket pads. The optional **Explore recorded
native settlement** opens the separate funded example and its native memo receipts.

## What is real and what is simulated

| Step | Behavior |
| --- | --- |
| Seller conditions | New bounded terms, random auction ID, conditions digest including cutoff, three fresh admission signing keys |
| Bid commitment / shares | Existing Poseidon2 commitment and REP3 bid/salt pairs, generated with fresh OS randomness |
| Input ZK proof | Existing Noir `bid_input` circuit and pinned plaindriver `--zk`; fixed C++ verifier checks the proof |
| Share admission | All three delivered pairs checked against the proof's exact public statement and intended conditions |
| Funding | **Synthetic**, unmined V3 note and fabricated journal Observe/Confirm events under a disjoint offline network genesis; no chain claim |
| Client intent | Existing signed claim, note/memo/receiver checks, using generated offline fixture identities |
| Acceptance | Three existing signed durable admission receipts combined and verified before the response |
| Closed book | Existing canonical ticket ordering/root and three signed endorsements, only after the declared cutoff |
| Fresh clearing | Existing three-party REP3 `delivery_flags4/v2`, fresh UltraHonk ZK proof and fixed C++ verification; public price/sold, blinded refund/status |
| Book continuity | Canonical ticket order, unchanged share values reindexed to those slots, exact first169 public fields matched against the signed certificate |
| Fresh own result | Only accepted ticket's pads opened after proof/certificate verification; local ticket capability, not native memo transport |
| Payment | **Recorded different auction**, unchanged native/public verification; no settlement of newly typed bids |

The localhost client helper **sees the typed demo maximum** while generating its
proof/shares. This is not in-browser witness generation, operator-blind browser
admission or a production wallet. The acceptance omits the maximum and private
opening, but that does not hide the input from this helper. All three application
keys/shares are on one host. Input proofs are single-client proofs; the separate
fresh clearing proof uses three actual REP3 processes. Privacy assumes at most
one corrupt semi-honest party, not malicious MPC security.

## Boundaries and restart

The API keeps loopback binding, strict Origin/session-token checks and bounded
JSON requests. Requests cannot select paths, keys, circuits, operator authorities,
RPC endpoints or canonical funding. The fixed input VK is prepared from the pinned
circuit, separately from the generated client artifacts. Maximum bids travel in
POST bodies and stdin to the helper, never command arguments or public replies.
Private logs/openings/keys/shares/journals stay under ignored mode0700 session
directories; secret files are mode0600. There is no browser local-storage export.

At most eight auctions/four tickets per auction are allowed in one server session.
Proof work uses the same server lock as recorded verification. Resource checks
run before each bounded subprocess; two Rayon threads and180-second command
timeouts are used for admission. Clearing/result workers use the existing owned
process-group guard with a600-second ceiling,4GiB disk/8GiB root-worker RSS/10%
system-memory headroom checks. This does not measure summed descendant RSS or
guarantee crash prevention. No six-row native proving is started.

If admission partially fails, the session stops that auction and preserves its
checkpoint instead of silently retrying a quorum. Restart intentionally creates
a new namespace; old local links are no longer active and ignored artifacts are
retained. Network-response/idempotent bid recovery and general wallet recovery
are not implemented. Opening capabilities stay in server/page memory and are
returned only with the corresponding successful admission; they are absent from
public GET/copy responses. Reload loses that page capability. This is a local
demonstration, not a production login. Copied acceptances contain public signatures
and the synthetic authority, never capabilities, viewing keys or bid openings.

Incomplete clearing is preserved and stops that auction. A completed clearing is
idempotent: another clear request returns the same verified result without
regenerating proofs. Own opening re-verifies proof and certificate before pads.

## Actual check

Against a running local demo:

```sh
python3 -B scripts/check_rehearsal.py --port 8877 \
  --output .local/rehearsal-check-FRESH.json
```

This makes four fresh input ZK proofs, twelve pair checks, twelve acceptance
signatures and three final-book signatures after a75-second real cutoff. It
rejects invalid terms/bids, Origin/token attacks, fifth/late tickets, early close
and actual proof/share/context substitutions before journal mutation. It does
not run fresh MPC clearing, native signing or RPC. The earlier measured run and browser
checks are in [entry evidence](../evidence/2026-10-05-seller-bidder-rehearsal.json).

For actual new-book REP3 clearing, four own results and tamper/capability/context
refusals (40-second real cutoff), run:

```sh
python3 -B scripts/check_rehearsal_clearing.py --port 8877 \
  --output .local/fresh-clearing-check.json
python3 -B scripts/check_rehearsal_clearing.py --port 8877 --zero \
  --output .local/fresh-zero-clearing-check.json
```

Run sequentially. Neither command funds or settles the new book; the plaintext
fixture oracle only checks independently computed MPC output.
