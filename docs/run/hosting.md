# Hosted live demo

The public site is one server: the landing at `/` and the live demo derived
from it at `/demo/settled` (the recorded testnet settlement, verified live) and
`/demo/run` (a new auction on public testnet). It is deployed at
**https://49-12-250-128.sslip.io** (see [Deployed](#deployed-oct-27) below).

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

1. Install Docker Engine, BuildKit (buildx) and the compose plugin on the server
   (Ubuntu 24.04: `apt-get install docker.io docker-buildx docker-compose-v2`).
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

## Deployed (Oct 27)

| | |
| --- | --- |
| URL | https://49-12-250-128.sslip.io (Let's Encrypt certificate through Caddy) |
| Server | Hetzner, x86_64 AMD, 8 vCPU, 15 GiB RAM (+ 8 GiB swap), 301 GB disk, Ubuntu 24.04 |
| Docker | Engine 29.1.3, buildx 0.30.1 (the image needs BuildKit for `TARGETARCH`) |
| Image | `darkcall-demo` from `git archive` of the deployed commit; first build 16 min with `JOBS=6` (co-snarks 310 s, Barretenberg 231 s, workspace 279 s), 8.05 GB |
| Halo2 parameters | `--prepare-params` ran at build time in 35 s; **Re-run verification** finishes in 17.8 s on the server (18.5 s seen from a browser) |
| Hosted checks | `scripts/check_hosted_http.py` through an SSH tunnel: all nine checks pass in 8.4 s |
| Parallel rows | not measured: 15 GiB is below the guard's 24 GiB for two workers |
| A new auction through the site | settled on public testnet on Oct 27: deposit `76329a49…`, settlement `03feb02b…`, 24 min 16 s from the landing page (clearing 6.8 s, settlement 11 min 11 s); [record](../../evidence/2026-10-27-hosted-testnet-auction.json) |
| Prover memory | the six funded rows peaked at 7.91 GiB against the default 8 GiB guard; `compose.yaml` sets `DARKCALL_MAX_PROVER_GIB=12` on this dedicated server |

The testnet bank's only note was mined at block 4,518,254 on Oct 19. Public
testnet captures now run near 10 KB a block, so rescanning from that birthday
(31,000 blocks) passed both the scanner's 100 MB read bound and its 20,000-block
window and every new auction would have failed at funding. A bank whose
birthday is more than 5,000 blocks old now deposits from its latest scan (any
past anchor stays valid, and `source_check` re-confirms the scan against the
chain), and each funding moves the bank's birthday to the block before its
deposit.

The first hosted runs also showed that tar created the private directories
0755 (the wallet tool needs 0700; the pack now carries 0700 entries), that a
deposit built from an old scan took that scan's expiry (an optional target
height now follows the tip), that the bank's last scan predated the Oct 21
deposit (`python3 -B scripts/testnet_chain.py rebase-bank TXID` restarts the
bank at a deposit), and that every MPC gate called macOS `time -l` (GNU time on
Linux now). One of those runs funded before the last fix and could not clear;
its 4 × 15,100 zatoshi stay in that auction's escrow, whose keys and journals
are kept on the server outside the served data.

