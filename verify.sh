#!/bin/sh
set -eu
TASK_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ "${1-}" = "--funded" ]; then
  shift
  exec python3 "$TASK_ROOT/scripts/funded_public_bundle.py" "$@"
fi
exec python3 "$TASK_ROOT/scripts/public_bundle.py" "$@"
