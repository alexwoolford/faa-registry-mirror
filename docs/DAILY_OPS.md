# FAA registry mirror (ops)

## Product

Third independent service on this host. Downloads the FAA Releasable Aircraft zip and stores MASTER / ACFTREF / ENGINE / DEREG / DOCINDEX / DEALER / RESERVED. Ownership and deregistration use SCD2; DEALER and RESERVED are full-replaced each run.

It is **not** registrant → ticker (that is tail-to-ticker) and **not** ADS-B trips (adsb-trip-journal). Dual FAA GETs with the producer are acceptable. This service does not import OpenSky or SEC credentials.

Join-friendly identifiers: `n_number` with a leading `N`, lowercase `icao24`. Dates are `YYYY-MM-DD`; write instants are `YYYY-MM-DDTHH:MM:SSZ`.

Do **not** use Docker. Do **not** run production from `$HOME` or cron. **The systemd timer is the scheduler.**

## Scheduler and telemetry

`faa-registry-mirror-ingest.timer` starts a `Type=oneshot` service. Do not add an in-process cron.

Operator logs: `tracing` on stderr → journald (`SyslogIdentifier=faa-registry-mirror-ingest`). Default `RUST_LOG=info`.

`ingest_runs` is capturable domain telemetry (status, row counts, timestamps). Query it in mosaic; do not scrape Prometheus from this oneshot.

## Cadence

| UTC | Job |
| --- | --- |
| ~05:30 | FAA nightly zip |
| **05:45 + ≤15m** | **this** ingest + publish `current/` |
| 06:00 + ≤15m | `adsb-trip-journal-collect` |
| 07:00 + ≤15m | `tail-to-ticker-refresh` |

## Layout

| Piece | Path |
| --- | --- |
| Binary / scripts | `/opt/faa-registry-mirror` (root:root) |
| Env | `/opt/faa-registry-mirror/etc/faa-registry-mirror.env` mode 600, `root:faa` |
| Work sqlite | `/var/lib/faa-registry-mirror/work/faa-registry.sqlite` |
| Zip cache | `/var/lib/faa-registry-mirror/cache/ReleasableAircraft.zip` (written after each origin GET; `--zip` reruns) |
| Published sqlite | `/var/lib/faa-registry-mirror/current/faa-registry.sqlite` mode 644 |

Failed or in-progress ingest leaves `current/` untouched. Publish is `VACUUM INTO` + `mv`.

## User-Agent

`registry.faa.gov` needs the Safari-like `FAA_DOWNLOAD_USER_AGENT` (or `FAA_USER_AGENT` override). The GitHub-style crate UA 403s on Akamai from this host.

**Never** send the FAA UA to `www.sec.gov`. This crate does not call SEC. The producer’s `SEC_USER_AGENT` is unrelated.

## Verify

```bash
systemctl is-enabled faa-registry-mirror-ingest.timer
systemctl list-timers 'faa-registry-mirror-*'
journalctl -u faa-registry-mirror-ingest.service -n 80 --no-pager

sudo -u faa /opt/faa-registry-mirror/scripts/run-status.sh

sqlite3 /var/lib/faa-registry-mirror/current/faa-registry.sqlite \
  "SELECT as_of_date, started_at, finished_at, status, master_rows
   FROM ingest_runs ORDER BY id DESC LIMIT 3;"

sqlite3 /var/lib/faa-registry-mirror/current/faa-registry.sqlite \
  "SELECT n_number, icao24 FROM aircraft WHERE is_current = 1 LIMIT 5;"
```

Expect `n_number` like `N…`, `icao24` lowercase hex, `as_of_date` `YYYY-MM-DD`, instants ending in `Z`, current MASTER on the order of ~300k.

Outbound HTTPS: `registry.faa.gov` only.

## State capture (prep)

Logical name: `faa-registry-mirror`. Watch the **work** sqlite ingest writes, not the published `current/` copy (`VACUUM INTO` / `mv` duplicates `_outbox`).

| Path | Role |
|---|---|
| `/var/lib/faa-registry-mirror/work/faa-registry.sqlite` | Watched. `_outbox` + triggers. |
| `/var/lib/faa-registry-mirror/current/faa-registry.sqlite` | Published snapshot. Do not watch. `open()` does not install capture. |

Capture set: `ingest_runs` (after), `aircraft` / `deregistered` (full; exclude `state_hash`), `documents` (after), `parse_errors` (after; exclude `raw_line`). Ref / dealers / reserved / FTS are zip-replaced or derived and are **not** captured. SCD close is `is_current=0` (a `U`); there is no `deleted_at` on aircraft.

Outbox/triggers come from [`capturable-state`](https://github.com/alexwoolford/capturable-state) `v0.1.0`, not a copied `capture.rs`.

Env (collector is `state-capture` on this host; missing socket is ignored):

```
STATE_CAPTURE_SOCK=/run/state/collect.sock
STATE_CAPTURE_ANNOUNCE_DIR=/var/lib/state-capture/announce
```

If the announce dir cannot be created, `open_work()` writes `{sqlite_dir}/.capturable.json`.
