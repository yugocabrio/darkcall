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
- Per-address limits: settlement verification 4 and receipt checks 12 per 10
  minutes, auction creation 3 per hour, other auction steps 240 and reads 1200
  per 10 minutes; then 429. One heavy job runs at a time (the existing proof
  lock); others get 409. Idle auctions are forgotten after three hours.
- The page says that on the hosted demo the bidder's client runs on the
  server, so the server sees the demo bid typed there; operators still get
  only shares and proofs.

## Server

Ubuntu 24.04, ARM64 preferred (the AES backend matches the measured host),
16 GiB RAM or more, 4 or more cores, 80 GB disk, ports 80 and 443 open. A
settlement peaks at about 3 GiB for the native prover and takes about 17 to
19 minutes on testnet. Examples: Oracle Cloud Always Free Ampere A1 (4 OCPU,
24 GB) or Hetzner CAX31 (8 vCPU, 16 GB).

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
5. Check `https://HOST/`, `/demo/settled` (open a receipt, run the tamper check,
   verify the settlement) and `/demo/run` (create an auction, seal a bid).

To update, rebuild the image from the new revision and run `docker compose up -d`
again; `.local` and the certificate volume persist.
