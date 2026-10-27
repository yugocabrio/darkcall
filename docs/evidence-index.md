# Evidence index

Every file in `evidence/` is a committed record of one run or check: its
commands, results and stated limits. Private material (keys, wallets, pads,
witnesses) is never committed; records cite public txids and commit hashes.

## Start here

| Topic | Record | What it shows |
| --- | --- | --- |
| Public Zcash testnet | [2026-10-27-hosted-testnet-auction](../evidence/2026-10-27-hosted-testnet-auction.json) | A new auction through the hosted live demo on a Linux server: deposit 76329a49…, settlement 03feb02b…, 24 min 16 s from the landing page; the five deployment fixes it needed |
| Public Zcash testnet | [2026-10-19-public-testnet-settlement](../evidence/2026-10-19-public-testnet-settlement.json) | Two auctions funded, cleared and settled on standard public testnet (NU7), one of them in the browser for the video; deposits and settlement txids with blocks |
| Public Zcash testnet | [2026-10-21-one-and-two-unit-settlement](../evidence/2026-10-21-one-and-two-unit-settlement.json) | One- and two-unit auctions with no circuit change; a one-unit auction settled on public testnet |
| Verification | [2026-10-18-clean-room-linux-verification](../evidence/2026-10-18-clean-room-linux-verification.json) | A clean Linux container built from git archive verified every committed bundle with no secrets and refused a corrupted proof |
| Verification | [2026-10-21-cheat-lab](../evidence/2026-10-21-cheat-lab.json) | A changed bid, price or payout proof each refused by the check it targets; an unchanged control passes |
| Verification | [2026-10-21-halo2-params-cache](../evidence/2026-10-21-halo2-params-cache.json) | Public verification 106 s to 18 s by caching fixed Halo2 parameters behind a pinned digest; proofs cross-verified |
| Verification | [2026-10-19-pre-publication-audit](../evidence/2026-10-19-pre-publication-audit.json) | Audit of the full git history for keys, wallets and private data before publication |
| Recovery | [2026-10-24-cancel-refund](../evidence/2026-10-24-cancel-refund.json) | Operator C stopped; A and B refunded all four deposits, 2-of-3 FROST, without C's journal or key share (regtest) |
| Recovery | [2026-10-05-dkg-funded-resumed-auction](../evidence/2026-10-05-dkg-funded-resumed-auction.json) | A funded DKG auction completed after an exact resume from the prepared transaction |
| Privacy | [2026-10-05-blinded-delivery-and-dictionary-attack](../evidence/2026-10-05-blinded-delivery-and-dictionary-attack.json) | A refund-guessing attack on the first delivery commitments, reproduced, and the blinded v2 fix |
| Funded pipeline | [2026-10-05-canonical-funded-nu7-memo-auction](../evidence/2026-10-05-canonical-funded-nu7-memo-auction.json) | A certified, funded NU7 memo auction with public and client replay (regtest) |
| Funded pipeline | [2026-10-18-fresh-book-native-settlement](../evidence/2026-10-18-fresh-book-native-settlement.json) | A book created in the browser settled through the funded pipeline on the same accepted root |
| Building blocks | [2026-10-04-native-poseidon2-output-binding-gate](../evidence/2026-10-04-native-poseidon2-output-binding-gate.json) | One BN254 Poseidon2 settlement row bound to a native Ironwood note commitment by Halo2 |
| Building blocks | [2026-10-04-external-verifier-gate](../evidence/2026-10-04-external-verifier-gate.json) | The jointly generated four-bid REP3 proof checked by the external C++ verifier |
| Measurements | [2026-10-21-bid-scaling](../evidence/2026-10-21-bid-scaling.json) | Gate counts at 4, 8 and 16 bids; the joint clearing proof measured at 4 and 8 bids |
| Measurements | [2026-10-05-warm-full-nu7-auctions](../evidence/2026-10-05-warm-full-nu7-auctions.json) | Two synthetic six-output auctions with one reused Halo2 proving key; per-stage timings |
| Spike | [2026-10-21-browser-bid-spike](../evidence/2026-10-21-browser-bid-spike.json) | The browser built the commitment, share pairs and proof; the unchanged admission accepted them |

## All records

The scope line is quoted from each record. Earlier records are often gates
whose limits a later record lifts; the record says which.

| Date | Record | Scope |
| --- | --- | --- |
| 2026-10-04 | [auction4-gate](../evidence/2026-10-04-auction4-gate.json) | synthetic calculation gate |
| 2026-10-04 | [auction4-reproduction](../evidence/2026-10-04-auction4-reproduction.json) | synthetic calculation gate |
| 2026-10-04 | [committed-input-gate](../evidence/2026-10-04-committed-input-gate.json) | synthetic committed-input calculation gate |
| 2026-10-04 | [cpu-native-mpc-sha-timeout](../evidence/2026-10-04-cpu-native-mpc-sha-timeout.json) | shared SHA-256 feasibility gate, not a complete native settlement proof |
| 2026-10-04 | [external-poseidon2-row-gate](../evidence/2026-10-04-external-poseidon2-row-gate.json) | 87aa5283d5b0cab6632ed49fdd25f6548d2ba91f |
| 2026-10-04 | [external-shared-row-verifier-gate](../evidence/2026-10-04-external-shared-row-verifier-gate.json) | 87aa5283d5b0cab6632ed49fdd25f6548d2ba91f |
| 2026-10-04 | [external-six-row-sha-bridge](../evidence/2026-10-04-external-six-row-sha-bridge.json) | 87aa5283d5b0cab6632ed49fdd25f6548d2ba91f |
| 2026-10-04 | [external-verifier-gate](../evidence/2026-10-04-external-verifier-gate.json) | matching external C++ verification of the committed four-bid REP3 proof |
| 2026-10-04 | [halo2-shared-sha-gate](../evidence/2026-10-04-halo2-shared-sha-gate.json) | standard SHA-256 with linked Halo2 message/state/feed-forward cells |
| 2026-10-04 | [mpc-poseidon2-row-gate](../evidence/2026-10-04-mpc-poseidon2-row-gate.json) | shared hash feasibility gate, not a complete native settlement proof |
| 2026-10-04 | [native-output-binding-gate](../evidence/2026-10-04-native-output-binding-gate.json) | one private row to actual native Ironwood output commitment |
| 2026-10-04 | [native-poseidon-domain-probes](../evidence/2026-10-04-native-poseidon-domain-probes.json) | native Poseidon codec domain feasibility |
| 2026-10-04 | [native-poseidon2-codec-binding-gate](../evidence/2026-10-04-native-poseidon2-codec-binding-gate.json) | BN254 Poseidon2 settlement row binds canonical recipient/amount to native Ironwood cmx |
| 2026-10-04 | [native-poseidon2-output-binding-gate](../evidence/2026-10-04-native-poseidon2-output-binding-gate.json) | BN254 Poseidon2 settlement row binds canonical recipient/amount to native Ironwood cmx |
| 2026-10-04 | [native-private-delivery-integration-gate](../evidence/2026-10-04-native-private-delivery-integration-gate.json) | offline synthetic native transaction gate |
| 2026-10-04 | [native-sha-output-binding-gate](../evidence/2026-10-04-native-sha-output-binding-gate.json) | SHA-256 settlement row binds canonical recipient/amount to native Ironwood cmx |
| 2026-10-04 | [native-transaction-gate](../evidence/2026-10-04-native-transaction-gate.json) | offline synthetic native transaction gate |
| 2026-10-04 | [native-transaction-mpc-frost-integration-gate](../evidence/2026-10-04-native-transaction-mpc-frost-integration-gate.json) | offline synthetic native transaction gate |
| 2026-10-04 | [native-zero-rows-gate](../evidence/2026-10-04-native-zero-rows-gate.json) | offline synthetic native transaction gate |
| 2026-10-04 | [non-native-poseidon2-hash-gate](../evidence/2026-10-04-non-native-poseidon2-hash-gate.json) | exact existing BN254 Poseidon2 sponge on Pallas, eight private elements |
| 2026-10-04 | [portable-mpc-sha-timeout](../evidence/2026-10-04-portable-mpc-sha-timeout.json) | shared SHA-256 feasibility gate, not a complete native settlement proof |
| 2026-10-04 | [private-delivery-public-bundle-replay](../evidence/2026-10-04-private-delivery-public-bundle-replay.json) | native-private-delivery-20261005 |
| 2026-10-04 | [private-refund-delivery-gate](../evidence/2026-10-04-private-refund-delivery-gate.json) | operator-only masked refund delivery |
| 2026-10-04 | [public-native-mpc-replay-gate](../evidence/2026-10-04-public-native-mpc-replay-gate.json) | public-only offline transaction/application-proof replay |
| 2026-10-04 | [shared-mpc-native-poseidon2-codec-row-gate](../evidence/2026-10-04-shared-mpc-native-poseidon2-codec-row-gate.json) | shared hash feasibility gate, not a complete native settlement proof |
| 2026-10-04 | [shared-mpc-native-poseidon2-row-gate](../evidence/2026-10-04-shared-mpc-native-poseidon2-row-gate.json) | shared hash feasibility gate, not a complete native settlement proof |
| 2026-10-04 | [shared-mpc-native-row-gate](../evidence/2026-10-04-shared-mpc-native-row-gate.json) | shared SHA-256 feasibility gate, not a complete native settlement proof |
| 2026-10-04 | [six-native-poseidon2-rows-gate](../evidence/2026-10-04-six-native-poseidon2-rows-gate.json) | six synthetic native outputs bound to Poseidon rows, reused key and sequential proofs |
| 2026-10-04 | [six-row-auction-native-poseidon2-bridge](../evidence/2026-10-04-six-row-auction-native-poseidon2-bridge.json) | six-row comparison with output-only native bundle |
| 2026-10-04 | [six-row-auction-native-sha-bridge](../evidence/2026-10-04-six-row-auction-native-sha-bridge.json) | six-row SHA baseline with output-only native bundle |
| 2026-10-04 | [six-row-native-sha-gate](../evidence/2026-10-04-six-row-native-sha-gate.json) | six synthetic native outputs bound to SHA settlement rows in one proof |
| 2026-10-04 | [test-chain-identification](../evidence/2026-10-04-test-chain-identification.json) | read-only TLS/gRPC provider identification; no funds or broadcast |
| 2026-10-05 | [blinded-delivery-and-dictionary-attack](../evidence/2026-10-05-blinded-delivery-and-dictionary-attack.json) | Public-only legacy dictionary attack; actual blinded-mask REP3/coSNARK refund and zero-price flag gates; offline historical-note exact native… |
| 2026-10-05 | [built-resume-gates](../evidence/2026-10-05-built-resume-gates.json) | pre-reservation exact native built-stage resume implementation; funded completion pending |
| 2026-10-05 | [canonical-funded-nu7-memo-auction](../evidence/2026-10-05-canonical-funded-nu7-memo-auction.json) | one actual-note certified NU7 memo auction, public/client replay; all actors on one controlled host |
| 2026-10-05 | [canonical-nu7-memo-auction](../evidence/2026-10-05-canonical-nu7-memo-auction.json) | ONE synthetic NU7 six-output auction including four encrypted receipts; not funded admission or chain inclusion |
| 2026-10-05 | [certified-public-settlement](../evidence/2026-10-05-certified-public-settlement.json) | Synthetic admission certificates linked to the demonstrated public-only MPC/native settlement; not chain funding |
| 2026-10-05 | [committed-auction-cases](../evidence/2026-10-05-committed-auction-cases.json) | synthetic committed-input REP3/ordinary verifier/Rust correctness regression |
| 2026-10-05 | [committed-review-kit](../evidence/2026-10-05-committed-review-kit.json) | Committed source packaging and same-machine extracted public-only smoke |
| 2026-10-05 | [cross-noir-reference](../evidence/2026-10-05-cross-noir-reference.json) | plaintext generated Rust sort oracle versus actual pinned Noir ACVM; not MPC or ZK |
| 2026-10-05 | [dependency-inventory](../evidence/2026-10-05-dependency-inventory.json) | declared package licenses and locked resolution; no legal compatibility claim |
| 2026-10-05 | [dkg-funded-resumed-auction](../evidence/2026-10-05-dkg-funded-resumed-auction.json) | actual funded isolated-regtest local-DKG auction after exact built-PCZT partial-proof resume |
| 2026-10-05 | [dkg-regtest-admission](../evidence/2026-10-05-dkg-regtest-admission.json) | actual isolated-regtest DKG escrow deposit/admission; native settlement still pending |
| 2026-10-05 | [encrypted-ticket-memos](../evidence/2026-10-05-encrypted-ticket-memos.json) | actual native encrypted memo transport component and fixed MPC ticket-opening verification; fabricated one-input/four-output transfers, not… |
| 2026-10-05 | [fresh-browser-clearing](../evidence/2026-10-05-fresh-browser-clearing.json) | Fresh synthetic browser book through actual same-book REP3 clearing and own result; recorded native payment remains separate |
| 2026-10-05 | [fresh-funded-blinded-v2](../evidence/2026-10-05-fresh-funded-blinded-v2.json) | fresh actual owned isolated-regtest deposits through blinded v2 auction and exact native DKG settlement; not public-testnet or mainnet |
| 2026-10-05 | [judge-demo-presentation](../evidence/2026-10-05-judge-demo-presentation.json) | Frontend hierarchy and recorded demo; no protocol redesign |
| 2026-10-05 | [local-public-settlement-verifier](../evidence/2026-10-05-local-public-settlement-verifier.json) | browser/API public-only replay of historical completed fresh-v2 isolated-regtest settlement; not new memo flow or live chain verification |
| 2026-10-05 | [local-receipt-demo](../evidence/2026-10-05-local-receipt-demo.json) | loopback synthetic receipt UI; separate transfer, not six-output memo settlement |
| 2026-10-05 | [native-binding-static-profile](../evidence/2026-10-05-native-binding-static-profile.json) | static working native row circuit profile, not measured memory or a proof |
| 2026-10-05 | [nested-mpc-resource-abort](../evidence/2026-10-05-nested-mpc-resource-abort.json) | owned synthetic nested-process abort regression and guarded actual REP3 winner delivery; no native chain mutation |
| 2026-10-05 | [nu7-auction-dependency-inventory](../evidence/2026-10-05-nu7-auction-dependency-inventory.json) | declared package licenses and locked resolution; no legal compatibility claim |
| 2026-10-05 | [nu7-binding-resource-stop](../evidence/2026-10-05-nu7-binding-resource-stop.json) | first new-SDK synthetic native output to working binding-circuit gate; host-resource stop before completion |
| 2026-10-05 | [nu7-byte-input-checks](../evidence/2026-10-05-nu7-byte-input-checks.json) | new-SDK public native cmx/index mapping and malformed private opening preflight; no completed application proof |
| 2026-10-05 | [nu7-compatibility](../evidence/2026-10-05-nu7-compatibility.json) | isolated SDK compatibility; working NU6.3 workspace unchanged; no public funding/broadcast |
| 2026-10-05 | [nu7-dependency-inventory](../evidence/2026-10-05-nu7-dependency-inventory.json) | declared package licenses and locked resolution; no legal compatibility claim |
| 2026-10-05 | [nu7-funded-admission](../evidence/2026-10-05-nu7-funded-admission.json) | fresh actual NU7 deposits and authenticated final book on isolated regtest; NOT six-output auction settlement |
| 2026-10-05 | [nu7-funded-construction-resource-stop](../evidence/2026-10-05-nu7-funded-construction-resource-stop.json) | funded NU7 same-PCZT integration implementation and initial resource stop; no completion claim |
| 2026-10-05 | [nu7-funded-dkg-transfer](../evidence/2026-10-05-nu7-funded-dkg-transfer.json) | actual NU7 native funding and DKG threshold transfer on a separate owned regtest; NOT the four-bid auction |
| 2026-10-05 | [nu7-funded-six-output-preparation](../evidence/2026-10-05-nu7-funded-six-output-preparation.json) | actual isolated NU7 funded auction preparation; no funded settlement proof/signature/inclusion |
| 2026-10-05 | [nu7-note-interop-dependencies](../evidence/2026-10-05-nu7-note-interop-dependencies.json) | declared package licenses and locked resolution; no legal compatibility claim |
| 2026-10-05 | [nu7-note-interop](../evidence/2026-10-05-nu7-note-interop.json) | standalone NU7/new-to-working V3 note byte gate; no workspace migration or public funding |
| 2026-10-05 | [nu7-six-output-preparation](../evidence/2026-10-05-nu7-six-output-preparation.json) | unsigned synthetic NU7 six-output preparation and pre-sign input checks; no completed auction binding |
| 2026-10-05 | [pc-reboot-recovery](../evidence/2026-10-05-pc-reboot-recovery.json) | PC-reboot audit of interrupted local DKG funded auction; completion pending |
| 2026-10-05 | [prepared-dkg-recovery](../evidence/2026-10-05-prepared-dkg-recovery.json) | offline historical actual-regtest note construction from copied journal prefixes; never broadcast |
| 2026-10-05 | [private-ticket-status-receipts](../evidence/2026-10-05-private-ticket-status-receipts.json) | synthetic per-ticket proof-authenticated openings; recipient network delivery/admission authentication unfinished |
| 2026-10-05 | [public-replay-prerequisites](../evidence/2026-10-05-public-replay-prerequisites.json) | public replay prerequisite inventory; not proof verification |
| 2026-10-05 | [recipient-memo-client](../evidence/2026-10-05-recipient-memo-client.json) | actual experimental recipient command; fixed C++ proof before own-key native memo decryption; synthetic transfers separate from auction settlement |
| 2026-10-05 | [regtest-funded-admission](../evidence/2026-10-05-regtest-funded-admission.json) | actual isolated-regtest four deposits and timely authenticated admission; fixed fixture terms and ticket IDs |
| 2026-10-05 | [regtest-funded-auction](../evidence/2026-10-05-regtest-funded-auction.json) | actual isolated regtest certified four-funded MPC/native auction |
| 2026-10-05 | [regtest-funded-threshold-spend](../evidence/2026-10-05-regtest-funded-threshold-spend.json) | actual isolated Zebra regtest threshold spend; not standard testnet or auction settlement |
| 2026-10-05 | [seller-bidder-rehearsal](../evidence/2026-10-05-seller-bidder-rehearsal.json) | Working localhost seller/bidder entry with fresh SINGLE-CLIENT input ZK proofs, verified REP3 share pairs and signed OFFLINE-SYNTHETIC admission;… |
| 2026-10-05 | [shared-binding-layout](../evidence/2026-10-05-shared-binding-layout.json) | Single actual new-SDK synthetic V3 output bound to BN254 row by separately identified shared18 Halo2 proof; no auction admission/funding/broadcast. |
| 2026-10-05 | [shared-layout-prepared-recovery](../evidence/2026-10-05-shared-layout-prepared-recovery.json) | offline shared18 v2 DKG recovery of exact historical PCZT/proofs with copied partial-Prepare journals; no new funded chain execution |
| 2026-10-05 | [shared-six-row-integration](../evidence/2026-10-05-shared-six-row-integration.json) | Explicit shared18 six-row synthetic native integration with blinded v2 refund delivery; no chain funding or broadcast |
| 2026-10-05 | [signed-recovery-gates](../evidence/2026-10-05-signed-recovery-gates.json) | bounded DKG rerun interruption and signed native restart implementation; end-to-end recovery pending |
| 2026-10-05 | [six-output-memo-preparation](../evidence/2026-10-05-six-output-memo-preparation.json) | actual unsigned six-output NU7 ticket memo preparation; complete native/application signing flow not yet run |
| 2026-10-05 | [testnet-nu7-identification](../evidence/2026-10-05-testnet-nu7-identification.json) | primary-source explanation of public testnet branch mismatch; no branch override or SDK migration |
| 2026-10-05 | [two-mode-demo-polish](../evidence/2026-10-05-two-mode-demo-polish.json) | Presentation only: two visibly separate experiences; no backend/API/circuit/authority/bundle changes |
| 2026-10-05 | [wallet-scanner-claims-receipts](../evidence/2026-10-05-wallet-scanner-claims-receipts.json) | Offline synthetic wallet library gates; no chain funding or broadcast |
| 2026-10-05 | [warm-full-nu7-auctions](../evidence/2026-10-05-warm-full-nu7-auctions.json) | two complete synthetic NU7 six-output memo auctions; one fixed PK, unchanged shared18 circuit and public statements |
| 2026-10-05 | [zero-price-native-v2](../evidence/2026-10-05-zero-price-native-v2.json) | zero-price committed auction with v2 private winner flags through exact native settlement; synthetic inputs/trusted dealer/offline |
| 2026-10-18 | [clean-room-linux-verification](../evidence/2026-10-18-clean-room-linux-verification.json) | clean-room Linux container verification of every committed funded settlement bundle; same physical Mac (Docker Desktop linux/arm64), not a second host |
| 2026-10-18 | [fresh-book-native-settlement](../evidence/2026-10-18-fresh-book-native-settlement.json) | fresh browser-created auction book settled on isolated Zebra 7.0.0-rc.0 NU7 regtest through the existing funded pipeline; synthetic demo bids, all… |
| 2026-10-19 | [pre-publication-audit](../evidence/2026-10-19-pre-publication-audit.json) | pre-publication audit of the full git history of this repository (all refs), before it is made public |
| 2026-10-19 | [public-testnet-settlement](../evidence/2026-10-19-public-testnet-settlement.json) | one fresh browser auction funded, cleared and settled on standard public Zcash testnet (NU7) through the demo API; synthetic demo bids; every… |
| 2026-10-21 | [bid-scaling](../evidence/2026-10-21-bid-scaling.json) | The committed circuits are fixed at four bids. How do the REP3 (coNoir) circuits and the settlement grow with N bids? |
| 2026-10-21 | [browser-bid-spike](../evidence/2026-10-21-browser-bid-spike.json) | Can the bidder's browser build the bid commitment, the REP3 share pairs and the bid_input proof, so the plaintext bid never reaches the server,… |
| 2026-10-21 | [cheat-lab](../evidence/2026-10-21-cheat-lab.json) | If an operator changes one thing in the recorded Oct 19 testnet settlement, does the public verifier notice, and which check refuses it? |
| 2026-10-21 | [halo2-params-cache](../evidence/2026-10-21-halo2-params-cache.json) | Where do funded settlement and public verification spend their time, and what can be removed without changing any circuit, proof, statement,… |
| 2026-10-21 | [one-and-two-unit-settlement](../evidence/2026-10-21-one-and-two-unit-settlement.json) | Can the existing four-bid protocol settle one- and two-unit auctions natively, with no circuit or proof change? |
| 2026-10-24 | [cancel-refund](../evidence/2026-10-24-cancel-refund.json) | If an operator stops before settlement, can the other two refund every deposit without the stopped operator, and without any chance of also… |
| 2026-10-27 | [hosted-testnet-auction](../evidence/2026-10-27-hosted-testnet-auction.json) | a new auction created, sealed, funded, cleared and settled through the hosted live demo (https://49-12-250-128.sslip.io) on standard public… |
