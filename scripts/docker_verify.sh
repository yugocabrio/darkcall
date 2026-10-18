#!/bin/sh
# Build the clean-room verifier from COMMITTED files only, then run it.
# Usage: scripts/docker_verify.sh [REVISION]   (default HEAD)
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REV=${1:-HEAD}
TAG=darkcall-verify:$(git -C "$ROOT" rev-parse --short "$REV")
git -C "$ROOT" archive --format=tar "$REV" | docker build -f docker/verify/Dockerfile -t "$TAG" -
exec docker run --rm "$TAG"
