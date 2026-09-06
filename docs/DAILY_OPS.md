# FAA registry mirror (ops)

## Product

Third independent service on this host. Downloads the FAA Releasable Aircraft zip and stores MASTER / ACFTREF / ENGINE / DEREG / DOCINDEX with SCD2 ownership history.

It is **not** registrant → ticker (that is tail-to-ticker) and **not** ADS-B trips (adsb-trip-journal). Dual FAA GETs with the producer are acceptable. This service does not import OpenSky or SEC credentials.

Join-friendly identifiers: `n_number` with a leading `N`, lowercase `icao24`. Dates are `YYYY-MM-DD`; write instants are `YYYY-MM-DDTHH:MM:SSZ`.

Do **not** use Docker. Do **not** run production from `$HOME` or cron.

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
