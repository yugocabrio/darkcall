# Hosted live demo

The public site is one server: the landing at `/` and the live demo derived
from it at `/demo/settled` (the recorded testnet settlement, verified live) and
`/demo/run` (a new auction on public testnet). Nothing is deployed until the
author provides the server and approves the deployment.

## What runs where

| Piece | On the server | Never on the server |
| --- | --- | --- |
| Code and tools | `darkcall-demo` image, built from a committed revision with `git archive`, every tool at the same pins as `docker/verify` | `.git`, local build outputs |
| Recorded auction | 11 files of the Oct 19 testnet run: transaction, identity, confirmation summary, receipt book and proof, four testnet recipient keys | logs, journals, escrow packages, PCZTs |
| New auctions | the testnet bank (testnet coins only); each run's files are created on the server | mainnet funds, the author's own keys or data |

`scripts/hosted_local_set.py --tar FILE` packs exactly those files (about 270
MiB, mostly the bank's note history) and refuses if any file names this
machine's home directory. With only that pack as `.local`, the recorded-mode
HTTP check passes: four receipts open, a corrupted proof is refused before any
key is read, and the settlement verifies.

## How it is served

- `scripts/serve_demo.py --public-origin https://HOST` binds 127.0.0.1:8877
  only. It accepts requests whose Host is HOST, and writes whose Origin is the
  public origin and which carry the session token.
- Caddy (`docker/demo/Caddyfile`) terminates TLS for HOST, forwards to the
  loopback server and replaces any client-supplied `X-Forwarded-For` with the
  real peer address. It writes no access log.
- Per-address limits: settlement verification 4, cheats 8 and receipt checks 12
  per 10 minutes, auction creation 3 per hour, other auction steps 240 and reads
  1200 per 10 minutes; then 429. Two locks: one new auction's funding or
  settlement at a time, and one settled-page check (receipt, verification or
  cheat) at a time; a second of either kind gets 409, but a running settlement
  does not block the settled page. A check peaks near 0.6 GiB and a settlement
  near 3 GiB. Idle auctions are forgotten after three hours.
- `scripts/check_hosted_http.py` checks a server in hosted mode as the proxy
  reaches it: the pages and icons, another Host refused, the hosted flag, a
  bidder receipt, the proof trail, a refused cheat, origin and token required,
  and the per-address limit. It funds no new auction.
- The page says that on the hosted demo the bidder's client runs on the
  server, so the server sees the demo bid typed there; operators still get
  only shares and proofs.

## Server

Ubuntu 24.04, ARM64 preferred (the AES backend matches the measured host),
16 GiB RAM or more, 4 or more cores, 80 GB disk, ports 80 and 443 open. A
settlement peaks at about 3 GiB for the native prover and took about 17 to
19 minutes on testnet before the fixed Halo2 parameters were cached (about
4 minutes less since; not re-measured on testnet). Examples: Oracle Cloud
Always Free Ampere A1 (4 OCPU, 24 GB) or Hetzner CAX31 (8 vCPU, 16 GB).

To also measure parallel payout proofs, prefer 8 vCPU and 32 GiB: the memory
guard allows `DARKCALL_ROW_WORKERS=2` only with at least 24 GiB total and
16 GiB available, and 3 with 32 GiB total and 24 GiB available (6 needs 56 and
48). It is checked before any deposit, so a smaller server refuses the setting
instead of stranding a funded auction.

After the first deploy, check the parameter cache: the image build log shows
`darkcall-native-gate --prepare-params`, and **Re-run verification** on
`/demo/settled` should finish in about 20 to 30 seconds, not about 2 minutes.

## Deploy

1. Install Docker Engine and the compose plugin on the server.
2. From this checkout, send the committed tree and build the image there:
   `git archive HEAD | ssh HOST 'mkdir -p darkcall && tar -x -C darkcall'`, then on
   the server `docker build -f darkcall/docker/demo/Dockerfile -t darkcall-demo:current darkcall`
   (or `scripts/hosted_demo_build.sh` where the repository is cloned).
3. Pack and send the private files:
   `python3 -B scripts/hosted_local_set.py --tar /tmp/darkcall-local.tar.gz`, copy it, then
   `sudo mkdir -m 700 /srv/darkcall-local && sudo tar -xzf darkcall-local.tar.gz -C /srv/darkcall-local && sudo chown -R 10001 /srv/darkcall-local`,
   and delete both copies of the tarball.
4. Start: `DARKCALL_HOST=<server-ip>.sslip.io docker compose -f darkcall/docker/demo/compose.yaml up -d`.
5. Check `https://HOST/`, `/demo/settled` (open a receipt, try each cheat,
   verify the settlement) and `/demo/run` (create an auction, seal a bid).

To update, rebuild the image from the new revision and run `docker compose up -d`
again; `.local` and the certificate volume persist.
