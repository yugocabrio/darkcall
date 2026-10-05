#!/bin/sh
set -eu
TASK_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ "${1-}" = "--synthetic" ]; then
  shift
  exec python3 "$TASK_ROOT/scripts/serve_demo.py" --canonical "$@"
fi
# Trusted local demo configuration, separately pinned from the proof bundle.
# User-supplied CLI options can override the run/bundle/authority deliberately.
exec python3 "$TASK_ROOT/scripts/serve_demo.py" --canonical --funded \
  --run nu7-funded-canonical-resumed-20261005 \
  --bundle "$TASK_ROOT/demo/artifacts/canonical-nu7-funded-v1" \
  --authority "$TASK_ROOT/demo/authority/canonical-nu7-funded-v1.json" "$@"
