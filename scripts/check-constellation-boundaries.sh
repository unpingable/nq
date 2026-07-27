#!/usr/bin/env bash
# Structural constellation boundary gate.  The Python implementation consumes
# the full resolved Cargo graph and also scans for private source-path coupling.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
exec python3 "$repo/scripts/check-constellation-boundaries.py" --repo "$repo" "$@"
