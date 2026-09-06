#!/usr/bin/env bash
# Install faa-registry-mirror under /opt and enable systemd (Linux).
# Usage (as root): ./deploy/install.sh
#
# Prefer building the release binary as a normal user first:
#   cargo build --release
#   sudo ./deploy/install.sh
# Set FORCE_REBUILD=1 to rebuild even when target/release/faa-registry-mirror exists.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="${FAA_INSTALL_PREFIX:-/opt/faa-registry-mirror}"
STATE="${FAA_STATE_DIR:-/var/lib/faa-registry-mirror}"
USER_NAME="${FAA_RUN_USER:-faa}"
GROUP_NAME="${FAA_RUN_GROUP:-$USER_NAME}"
BIN_SRC="$ROOT/target/release/faa-registry-mirror"
ENV_DST="$PREFIX/etc/faa-registry-mirror.env"

if [[ "$(id -u)" -ne 0 ]]; then
  echo "run as root" >&2
  exit 1
fi

build_release() {
  local build_user="${SUDO_USER:-}"
  if [[ -n "$build_user" && "$build_user" != "root" ]] && id -u "$build_user" >/dev/null 2>&1; then
    echo "== build release (as $build_user) =="
    sudo -u "$build_user" -H bash -lc "cd \"$ROOT\" && source \"\$HOME/.cargo/env\" 2>/dev/null || true; cargo build --release"
    return
  fi
  echo "no release binary at $BIN_SRC and no non-root SUDO_USER to build as." >&2
  echo "build first: cargo build --release" >&2
  echo "then re-run: sudo ./deploy/install.sh" >&2
  exit 1
}

if [[ -x "$BIN_SRC" && -z "${FORCE_REBUILD:-}" ]]; then
  echo "== using existing release binary: $BIN_SRC =="
else
  build_release
fi

test -x "$BIN_SRC" || {
  echo "missing $BIN_SRC — build with: cargo build --release" >&2
  exit 1
}

echo "== create user/dirs =="
NLOGIN="/usr/sbin/nologin"
[[ -x "$NLOGIN" ]] || NLOGIN="/sbin/nologin"
if ! id -u "$USER_NAME" >/dev/null 2>&1; then
  useradd --system --home-dir "$STATE" --shell "$NLOGIN" "$USER_NAME" || true
fi
mkdir -p "$PREFIX"/{bin,scripts,etc,docs} \
  "$STATE"/work \
  "$STATE"/cache \
  "$STATE"/current \
  /etc/systemd/system

echo "== install files =="
install -m 0755 "$BIN_SRC" "$PREFIX/bin/faa-registry-mirror"
install -m 0755 "$ROOT/scripts/run-ingest.sh" "$PREFIX/scripts/run-ingest.sh"
install -m 0755 "$ROOT/scripts/run-status.sh" "$PREFIX/scripts/run-status.sh"
install -m 0644 "$ROOT/docs/DAILY_OPS.md" "$PREFIX/docs/DAILY_OPS.md"

if [[ ! -f "$ENV_DST" ]]; then
  if [[ -n "${FAA_ENV_FILE:-}" && -f "$FAA_ENV_FILE" ]]; then
    install -m 0600 "$FAA_ENV_FILE" "$ENV_DST"
  else
    install -m 0600 "$ROOT/deploy/faa-registry-mirror.env.example" "$ENV_DST"
  fi
fi
chmod 0600 "$ENV_DST"

chown -R "$USER_NAME:$GROUP_NAME" "$STATE"
chown -R root:root "$PREFIX"
chown root:"$GROUP_NAME" "$PREFIX/etc" "$ENV_DST"
chmod 0750 "$PREFIX/etc"
chmod 0600 "$ENV_DST"
chmod 0755 "$PREFIX" "$PREFIX/bin" "$PREFIX/scripts" "$PREFIX/docs"
chmod 0755 "$PREFIX/scripts"/*.sh
chmod 0755 "$STATE" "$STATE/current"
chmod 0750 "$STATE/work" "$STATE/cache"

install -m 0644 "$ROOT/deploy/systemd/faa-registry-mirror-ingest.service" \
  /etc/systemd/system/faa-registry-mirror-ingest.service
install -m 0644 "$ROOT/deploy/systemd/faa-registry-mirror-ingest.timer" \
  /etc/systemd/system/faa-registry-mirror-ingest.timer

if command -v restorecon >/dev/null 2>&1; then
  echo "== SELinux restorecon =="
  restorecon -Rv "$PREFIX" "$STATE" || true
fi

systemctl daemon-reload
systemctl enable --now faa-registry-mirror-ingest.timer

echo "installed:"
echo "  prefix=$PREFIX state=$STATE"
echo "  publish: $STATE/current/faa-registry.sqlite"
echo "  logs: journalctl -u faa-registry-mirror-ingest.service"
echo "  timer: faa-registry-mirror-ingest.timer enabled (daily 05:45 UTC + 15m jitter)"
echo "  env: $ENV_DST (chmod 600; empty FAA_USER_AGENT uses the Safari token)"
