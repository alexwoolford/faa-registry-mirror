#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${FAA_REGISTRY_BIN:-$ROOT/bin/faa-registry-mirror}"
PUBLISH="${FAA_REGISTRY_PUBLISH:-/var/lib/faa-registry-mirror/current/faa-registry.sqlite}"

test -x "$BIN" || {
  echo "missing $BIN" >&2
  exit 1
}
test -s "$PUBLISH" || {
  echo "no published sqlite at $PUBLISH" >&2
  exit 1
}

exec "$BIN" --db "$PUBLISH" status
