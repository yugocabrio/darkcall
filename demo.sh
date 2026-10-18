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
# Trusted local demo configuration, separately pinned from the proof bundle.
# "Proof it settles" shows the auction that was created, funded, cleared and
# settled through this UI on Oct 19 (tx e3ffa436...84769754). Without its
# ignored local run, the same committed bundle is served public-only.
exec python3 "$TASK_ROOT/scripts/serve_demo.py" --canonical --funded \
  --run settle-35b2a2569d-settlement \
  --bundle "$TASK_ROOT/demo/artifacts/browser-auction-nu7-v2" \
  --authority "$TASK_ROOT/demo/authority/browser-auction-nu7-v2.json" \
  --recorded-note "Created, funded, cleared and settled in this UI on Oct 19 · isolated Zcash regtest" "$@"
