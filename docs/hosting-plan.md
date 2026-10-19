# Judge access and hosting plan

Nothing here is deployed. Any public hosting, server purchase or repository
visibility change needs the author's approval.

## What a judge can do today without installing anything

| Time | What | Where |
| --- | --- | --- |
| 20 s | See the difference: a typical audited auction opens bids; DarkCall proves the price without opening them | README top, `docs/img/01-landing.jpg` |
| 2 min | Watch one real auction from creation to shielded payout, with labeled time cuts | Edited demo video (rendered from `demo/media/v2/`, see [demo.md](demo.md)) |
| 2 min | Read the exact run: deposit and settlement txids, root, timings, refusals | [evidence](../evidence/2026-10-18-fresh-book-native-settlement.json) |

Running anything needs the pinned toolchain. The lightest real check is
`scripts/docker_verify.sh`. It needs Docker and about 15 GB of disk, and takes
about 20 minutes to build. It verifies every committed settlement bundle and
rejects a tampered proof.

## Three tiers, by cost and risk

| Tier | What runs | Resources | Fits an 8 GB VPS? | Main risks |
| --- | --- | --- | --- | --- |
| A. Static evidence viewer | HTML only: screenshots, video, facts, commands | none | yes (any static host, e.g. GitHub Pages once public) | none beyond publishing the repo |
| B. Public verifier | `serve_demo.py --public-only`: open the recorded auction, run the public settlement check and the tamper check; no keys, no bids | ~2 min CPU and a few GB RAM per check | yes, with one job at a time | CPU exhaustion by repeated clicks; must not expose private runs |
| C. Interactive MPC | create, seal bids, clear by REP3, open own result; no Fund/Settle | <1 s per bid, about 9 s per clearing, ~2 GB | probably, one auction at a time | the server's helper sees every typed demo bid; must be labeled as such |
| D. Full settlement | Fund + Settle with Zebra regtest and FROST | peak prover RSS up to ~8 GB, 10 GB disk headroom, about 17 min per auction | **no** | memory guard aborts; keep local, show recorded evidence |

**Tier A is built (Oct 19).** `docs/index.html` and `docs/site/` form a static
evidence page: the video, the one-auction walkthrough with real frames and
timestamps, both transaction IDs, who learned what, the verify commands, the
bundle table and the trust model. `python3 -B scripts/build_evidence_site.py`
generates it from the capture manifest and the evidence JSON, so nothing is
typed by hand. It needs no scripts or external resources. It was previewed
locally at 1280 px and 390 px. To publish after the repository is public:
Settings → Pages → Deploy from a branch → `main` / `/docs`. That is the
author's step; nothing is deployed.

**Recommendation.** Ship tier A when the repository becomes public. Offer B only
if the author wants a live verify button for judges. Keep C and D on the local
machine, used for the video and live demos.

## Required changes before tier B or C (not implemented)

The current server is deliberately loopback-only. It must not be exposed to the
Internet unchanged.

- **Origin and Host.** Requests are accepted only for Host `127.0.0.1:PORT`.
  Behind a TLS reverse proxy, add an explicit configured public origin; never
  disable the check.
- **No private runs.** Start only with `--public-only` and copy only committed
  public bundles and authorities to the host. No `.local`, wallets, FVK records
  or journals.
- **Admission control.** Keep the single proof lock, add a per-IP rate limit at
  the proxy, cache the last verification result and bound the queue.
- **Tier C only.** Disable Fund/Settle (no regtest node or bank on the host) and
  show "this server sees your demo bid" before input. Restart regularly so
  rehearsal state does not accumulate.
- **Operations.** Run under a dedicated unprivileged user with read-only source
  and keep logs private. Provide no shell or file upload paths; the API already
  accepts no paths.
