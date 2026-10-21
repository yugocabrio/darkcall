#!/bin/sh
set -eu
TASK_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ "${1-}" = "--synthetic" ]; then
  shift
  exec python3 "$TASK_ROOT/scripts/serve_demo.py" --canonical "$@"
fi
if [ "${1-}" = "--fixture-recorded" ]; then
  # The Oct 5 recorded settlement with fixture bids (tx 47019e02...a211938).
  shift
  exec python3 "$TASK_ROOT/scripts/serve_demo.py" --canonical --funded \
    --run nu7-funded-canonical-resumed-20261005 \
    --bundle "$TASK_ROOT/demo/artifacts/canonical-nu7-funded-v1" \
    --authority "$TASK_ROOT/demo/authority/canonical-nu7-funded-v1.json" "$@"
fi
if [ "${1-}" = "--regtest-recorded" ]; then
  # The six-step regtest auction created, funded, cleared and settled in this UI
  # on Oct 19 (tx e3ffa436...84769754).
  shift
  exec python3 "$TASK_ROOT/scripts/serve_demo.py" --canonical --funded \
    --run settle-35b2a2569d-settlement \
    --bundle "$TASK_ROOT/demo/artifacts/browser-auction-nu7-v2" \
    --authority "$TASK_ROOT/demo/authority/browser-auction-nu7-v2.json" \
    --ticket-order "$TASK_ROOT/demo/ticket-order/browser-auction-nu7-v2.json" \
    --recorded-note "Created, funded, cleared and settled in this UI on Oct 19 · isolated Zcash regtest" "$@"
fi
# Trusted local demo configuration, separately pinned from the proof bundle.
# "See a settled auction" shows the auction in the video: created, funded,
# cleared and settled in this UI on public Zcash testnet on Oct 19 (tx
# be1157c8...a1c8ad67). Without its ignored local run, the same committed bundle
# is served public-only. Add --chain testnet to run Fund and Settle for new
# auctions on testnet as well.
exec python3 "$TASK_ROOT/scripts/serve_demo.py" --canonical --funded \
  --run settle-aa36c45a6a-settlement \
  --bundle "$TASK_ROOT/demo/artifacts/testnet-auction-nu7-v2" \
  --authority "$TASK_ROOT/demo/authority/testnet-auction-nu7-v2.json" \
  --ticket-order "$TASK_ROOT/demo/ticket-order/testnet-auction-nu7-v2.json" \
  --recorded-note "Created, funded, cleared and settled in this UI on public Zcash testnet on Oct 19" "$@"
