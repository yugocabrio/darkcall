#!/bin/sh
# Build the hosted demo image from COMMITTED files only and tag it current.
# Usage: scripts/hosted_demo_build.sh [REVISION]   (default HEAD)
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REV=${1:-HEAD}
TAG=darkcall-demo:$(git -C "$ROOT" rev-parse --short "$REV")
git -C "$ROOT" archive --format=tar "$REV" | docker build -f docker/demo/Dockerfile -t "$TAG" -
docker tag "$TAG" darkcall-demo:current
echo "$TAG"
