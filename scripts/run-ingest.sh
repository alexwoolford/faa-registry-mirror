#!/usr/bin/env bash
# Oneshot: download the FAA zip, ingest into work/, atomically publish current/.
# A failed ingest leaves /var/lib/faa-registry-mirror/current/ untouched.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${FAA_REGISTRY_BIN:-$ROOT/bin/faa-registry-mirror}"
WORK="${FAA_REGISTRY_WORK:-/var/lib/faa-registry-mirror/work}"
CACHE="${FAA_REGISTRY_CACHE:-/var/lib/faa-registry-mirror/cache}"
PUBLISH="${FAA_REGISTRY_PUBLISH:-/var/lib/faa-registry-mirror/current/faa-registry.sqlite}"
LOCK="${FAA_REGISTRY_LOCK:-/var/lib/faa-registry-mirror/.ingest.lock}"
SRC="${WORK}/faa-registry.sqlite"

test -x "$BIN" || {
  echo "missing $BIN — build with: cargo build --release" >&2
  exit 1
}

mkdir -p "$WORK" "$CACHE" "$(dirname "$PUBLISH")"

acquire_lock() {
  if command -v flock >/dev/null 2>&1; then
    exec 9>"$LOCK"
    if ! flock -n 9; then
      echo "ingest already running (lock $LOCK)" >&2
      exit 1
    fi
  else
    if ! mkdir "$LOCK.d" 2>/dev/null; then
      echo "ingest already running (lock $LOCK.d)" >&2
      exit 1
    fi
    trap 'rmdir "$LOCK.d" 2>/dev/null || true' EXIT
  fi
}
acquire_lock

echo "== faa-registry-mirror ingest =="
echo "bin=$BIN work=$WORK publish=$PUBLISH"
"$BIN" --db "$SRC" ingest

test -s "$SRC" || {
  echo "missing $SRC" >&2
  exit 1
}

TMP="${PUBLISH}.tmp"
rm -f "$TMP"
if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 is required to VACUUM INTO a consistent publish snapshot" >&2
  exit 1
fi
sqlite3 "$SRC" "PRAGMA busy_timeout=5000; VACUUM INTO '$TMP';" >/dev/null
chmod 0644 "$TMP"
mv -f "$TMP" "$PUBLISH"
echo "published → $PUBLISH"
