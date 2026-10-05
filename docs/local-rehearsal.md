# Seller and bidder entry rehearsal

The browser now has a working entry loop before the recorded settlement:

**Create → share locally → seal a demo bid → signed acceptance → certify the
closed book → explore recorded clearing → own result → verify / tamper.**

The entry and recorded settlement are **different auctions**. New inputs have
fresh conditions, tickets, randomness and admission keys. They do not become
part of transaction47019e02…a211938, select its winning ticket or produce a new
MPC clearing/native settlement. The transition and recorded result banner state
this explicitly. The existing funded transaction/proofs and verifier remain intact.

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
the winners. **View recorded clearing & settlement** opens the separate completed
funded example; select an example bidder to open that transaction's receipt.

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
| Clearing / payment | **Recorded different auction**, unchanged public/recipient verification; no settlement of newly typed bids |

The localhost client helper **sees the typed demo maximum** while generating its
proof/shares. This is not in-browser witness generation, operator-blind browser
admission or a production wallet. The acceptance omits the maximum and private
opening, but that does not hide the input from this helper. All three application
keys/shares are on one host. The new input proofs are single-client proofs; they
are not proof that a fresh REP3 auction ran.

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
timeouts are used. This is not continuous descendant-RSS guarding or a production
service. No heavy six-row proving is started by this entry flow.

If admission partially fails, the session stops that auction and preserves its
checkpoint instead of silently retrying a quorum. Restart intentionally creates
a new namespace; old local links are no longer active and ignored artifacts are
retained. Network-response/idempotent bid recovery and general wallet recovery
are not implemented. Copied acceptances contain public signatures and the
synthetic authority, never a viewing key or a bid opening.

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
not run fresh MPC clearing, native signing or RPC. The measured run and browser
checks are in [entry evidence](../evidence/2026-10-05-seller-bidder-rehearsal.json).
