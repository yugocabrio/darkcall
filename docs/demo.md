# Local receipt demo

Start from the repository root:

```sh
python3 scripts/serve_demo.py
```

Open `http://127.0.0.1:8877`. The server binds only to IPv4 loopback. It serves
three static files and a bounded API; there is no repository file server, CORS,
remote host option, signing, RPC or transaction broadcast operation.

Choose standard clearing or the zero-price tie, then a synthetic ticket.
**Verify & open my result** runs the actual one-command recipient client:
fixed C++ verification before reading the recipient key, native transaction
authentication/decryption, and exact ticket/root/pad/refund checks. The browser
receives the verified result only, never viewing keys, spending keys or pads.
**Run tamper check** changes a proof and requires rejection before reading the
recipient key or decrypting the memo. It does not simulate a successful result.

The demo is the completed one-input/four-output encrypted receipt component.
It is separate from six-output auction settlement. The newer same-six-output
memo PCZT preparations have passed, but their complete NU7 application/native
proof and signing run is still capacity-blocked. There is no new bid form:
sending a plaintext maximum bid to this server would violate the intended
privacy model. Independent browser bid proving/admission is unfinished.

The server uses **synthetic identities on this one computer**. Selecting a demo
ticket is not bidder authentication. Operators know winning status/refunds.
This is not a deployed wallet, independent-host privacy demonstration, admission
receipt or proof of chain inclusion.

**Verify settlement** independently runs the public-only verifier on the
completed price-25 six-output owned-regtest auction. It snapshots public
transaction/row/MPC proofs, supplies the original public operator authority
from outside that bundle, and verifies all three acceptance signatures, six
native bindings, native authorization and the fixed C++ auction proof. It
reads no bid openings, wallet keys, PCZT or funding journal. This historical
settlement has no receipt memos; this check does not reestablish chain inclusion
or current spendability. A verification takes about two minutes on this host.
Its fixed checkpoint/authority paths are in `serve_demo.py`, not browser input.

## Fixture prerequisites

Default fixtures are private ignored checkpoints from the measured runs:

- `.local/positive-receipt-memo-20261005`
- `.local/zero-receipt-memo-v2-20261005`

They are not in Git. On a new machine first build the pinned tools from
[toolchains.md](toolchains.md), then run `scripts/run_mpc_gate.py` with synthetic
bids and `scripts/run_delivery_gate.py --winner-flags` for the returned book.
For standard clearing use bids `90 70 60 50`; for the zero tie use
`--reserve 0 --bids 0 0 0 0`. Both scripts return their new ignored `run` path.
Generate each native receipt fixture with:

```sh
python3 scripts/check_receipt_transport.py BOOK_RUN FLAGS_RUN --name NEW_RECEIPT_NAME
```

Supply the **directory names** of those generated checkpoints to
`serve_demo.py --positive POSITIVE_RECEIPT_NAME --zero ZERO_RECEIPT_NAME`.
Existing checkpoints are preserved; use fresh names. Missing/wrong-scope
fixtures fail before the server binds. Each recipient opening uses the existing
4GiB disk/8GiB RSS/10% system-memory guard. The complete new six-row prover
requires10GiB initial disk headroom; the UI never lowers that requirement.

## Validation

```sh
python3 scripts/check_demo_http.py
python3 scripts/check_demo_http.py --settlement-security-only
python3 scripts/check_demo_http.py --settlement-only
```

The actual check covers four positive/zero winner/loser results, two corrupted
proof refusals,15 HTTP/input rejections and secret-field exclusion. Browser
interaction separately confirms result rendering, identity selection and the
tamper rejection. UI refresh resets the presentation only; private proof/client
checkpoints stay under `.local` for review/recovery.

The API accepts only allowlisted synthetic case/ticket/action values, exact
loopback Host and same Origin, a per-server session token, and bounded JSON.
One verification can run at a time. This provides local demo isolation, not a
public service authentication protocol. All files/diagnostics remain local.
