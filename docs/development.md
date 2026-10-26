# Development record

DarkCall was built for ZECATHON 2026 between October 4 and the October 28
deadline. This page lists what was built when, with the record behind each
step, and what is not done. Every record in `evidence/` states its own commands
and limits.

## Milestones

| Date | What was built | Record |
| --- | --- | --- |
| Oct 4 | Pinned toolchain (Noir 1.0.0-rc.2, TACEO co-snarks, Barretenberg); the bid input circuit; first REP3 and native-signing experiments | [dependencies](dependencies.md) |
| Oct 5 | Four-bid REP3 clearing with a joint coNoir proof; six Halo2 output bindings; a funded NU7 regtest auction signed 2-of-3 with DKG FROST; blinded refund delivery after reproducing a refund-guessing attack; crash and resume gates | [funded auction](../evidence/2026-10-05-canonical-funded-nu7-memo-auction.json), [attack and fix](operator-fill-delivery.md), [resume](../evidence/2026-10-05-dkg-funded-resumed-auction.json) |
| Oct 18 | Books created in the browser settled on the same accepted root; clean-room Linux verification from `git archive` | [browser books](../evidence/2026-10-18-fresh-book-native-settlement.json), [clean room](../evidence/2026-10-18-clean-room-linux-verification.json) |
| Oct 19 | Funding before clearing; the six-step browser flow; two auctions settled on public Zcash testnet, one of them in the browser for the video | [public testnet](../evidence/2026-10-19-public-testnet-settlement.json) |
| Oct 21 | Clearing measured at 8 bids; hosted mode; the cheat lab and proof trail; one- and two-unit settlement, with a one-unit auction on testnet; bids sealed in the browser (spike); cached Halo2 parameters (public check 106 s to 18 s) | [scaling](../evidence/2026-10-21-bid-scaling.json), [cheat lab](../evidence/2026-10-21-cheat-lab.json), [units](../evidence/2026-10-21-one-and-two-unit-settlement.json), [browser spike](../evidence/2026-10-21-browser-bid-spike.json), [parameters](../evidence/2026-10-21-halo2-params-cache.json) |
| Oct 24 | Refund of every deposit when an operator stops before settlement, signed by the other two | [refund](../evidence/2026-10-24-cancel-refund.json) |
| Oct 26 | Judge-facing landing: the problem, what each party sees, what happens when something fails, the technical relations and the evidence | landing page |

## Not done

- The hosted public demo is prepared (`docker/demo`) but not deployed.
- The operator-stopped refund has run on regtest only, not on public testnet.
- Parallel payout proofs (`DARKCALL_ROW_WORKERS`) are wired behind the Linux
  memory guard but not measured.
- Bids sealed in the browser are a spike; the demo's bid step uses the bidder's
  local client helper.
- Auctions have four bids; an 8-bid settlement is not planned.
- REP3 is semi-honest, two colluding operators can reconstruct bids, and the
  demo runs every role on one host. The full list is in the README's trust model.

## How to check

```sh
./verify.sh --funded demo/artifacts/testnet-auction-nu7-v2 \
  --authority demo/authority/testnet-auction-nu7-v2.json   # public verification, no secrets
scripts/docker_verify.sh                                      # clean-room build and every bundle
python3 -B scripts/cheat_lab.py none book price payout        # each dishonest change refused
python3 -B scripts/check_fresh_settlement.py --port 8877 --output .local/run.json   # a full auction over HTTP
python3 -B scripts/check_cancel_refund.py --port 8877 --output .local/refund.json   # the operator-stopped refund (regtest)
```

Continuous integration runs the Noir, Rust and Python test suites on every push.
