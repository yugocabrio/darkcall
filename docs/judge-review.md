# DarkCall — private bids, verifiable shielded settlement

Four bidders submit secret maximum bids for three identical units. DarkCall
computes one clearing price without reconstructing the bid list, pays the seller
and returns shielded refunds. Each bidder decrypts an authenticated result from
the memo of their own refund note. A public proof package checks that the auction
result and all six outputs belong to the same Zcash transaction.

**One funded NU7 auction completes on an isolated regtest.** Transaction
`47019e02e49557ca1e8ef899b94ff1e464a9f29ec14accc298fb0bad3a211938`
connects four actual deposits, client/share checks, a signed final book, REP3
clearing, four encrypted receipts, six native bindings and 2-of-3 DKG signing.
Zebra accepted it; two controlled confirmations, six exact recipient scans,
four spent source notes, zero escrow balance and three finalized journals were
recorded. The public bundle is 153,557 bytes.

## Review in three steps

In this archive, `source/` contains the committed project. In a Git checkout,
use the repository root directly. The archive carries source and public
artifacts; it carries no dependency binaries or private fixture identities.

1. Read `source/docs/submission-draft.md` for the product and threat model,
   and `source/evidence/2026-10-05-canonical-funded-nu7-memo-auction.json`
   for exact measurements and the controlled-chain record.
2. From `source/`, run `./demo.sh --public-only`. This starts a loopback browser
   view with no recipient keys. Opening private results requires an owner's
   own viewing identity, which is deliberately excluded from the archive.
3. Build the pinned tools using `docs/reproduce.md`, run the preflight and then:

   ```sh
   ./verify.sh --funded demo/artifacts/canonical-nu7-funded-v1 \
     --authority demo/authority/canonical-nu7-funded-v1.json
   ```

The authority is **separate caller-trusted demo configuration**. Obtain the
intended authority independently in a deployment; a producer-provided archive
does not establish trust in its own authority keys. Manifest hashes detect
packaging damage; pinned native/Halo2/C++ verifiers establish proof validity.
The public verifier needs no wallet key, PCZT, private bid, mask or witness.

## What is demonstrated, and what is assumed

| Question | Answer |
| --- | --- |
| Can one operator see the bid list? | REP3 keeps inputs shared under an at-most-one semi-honest corruption assumption. Two colluding operators can reconstruct it. |
| Can operators see who won? | Yes. Operators receive refunds/status and have escrow viewing capability. This is confidential maximum-bid computation, not operator-blind outcomes. |
| Who controls funds? | Two of three DKG signers. Two colluders can move funds; there is no unilateral user refund. All three currently run on one host. |
| What does admission prove? | Operator signatures attest funding and the accepted book. This is not a public ZK funding proof. |
| Does the public package prove confirmation? | No. Chain acceptance/scans are a separate controlled regtest observation. Public verification checks authorization, output bindings and auction/certificate validity. |
| Is item delivery covered? | No. Goods, identity/Sybil control and network anonymity are outside this prototype. |
| Is public testnet / mainnet demonstrated? | No. The completed funded run is one-host isolated regtest. |

The earlier unblinded delivery commitment leaked two-candidate refunds; the
attack was reproduced and its privacy claim withdrawn. Current v2 uses uniform
fresh masks plus independent secret commitment blinders. The circuit does not
prove CSPRNG quality. Details: `source/docs/operator-fill-delivery.md`.

## Latency and remaining work

Actual REP3 witness/proof: 1.423s / 10.824s. Full funded recovery: 1847.653s on
a shared 16GiB M1 with one native thread, excluding a separate 190.563s disk-guard
stop. Six sequential native row services (1295.846s) dominate. Public replay:
112.431s. Warm PK reuse works; Linux 2/3/6-way speedup remains unmeasured.
No two-minute settlement or independent-machine reproduction is claimed.

Remaining: second-machine reproduction, public testnet, independent operators,
larger bid counts, general live-wallet recovery and live browser admission.
The present four-slot/six-output statements and verifiers are frozen for demo
and reproduction. See `source/STATUS.md`, `source/TASKS.md` and licenses in
`source/LICENSE`, `source/vendor/halo2_gadgets/` and `source/docs/dependencies.md`.

This kit is prepared locally for review. Repository publication, external
deployment and contest submission remain under the human's control.
