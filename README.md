# FAA Aircraft Ownership Mirror

Rust CLI (`faa-registry-mirror`) that downloads the FAA [Releasable Aircraft Database](https://registry.faa.gov/database/ReleasableAircraft.zip), parses it with the official fixed-width field map from [ardata.pdf](https://www.faa.gov/licenses_certificates/aircraft_certification/aircraft_registry/media/ardata.pdf), and stores ownership in a local SQLite file.

Daily re-runs apply deltas only. Ownership and deregistration rows use Slowly Changing Dimensions (Type 2): a hash change closes the old row and inserts a new current one, so you can ask who owned an N-number last year.

On the production host this is a third systemd service (user `faa`), not Docker and not `$HOME` cron. See [docs/DAILY_OPS.md](docs/DAILY_OPS.md).

## Commands

```bash
# First load (downloads ~60MB zip). Default DB: data/faa-registry.sqlite
cargo run --release -- ingest

# Later updates — same zip_hash is skipped unless --force
cargo run --release -- ingest --force

# Or ingest a zip you already have
cargo run --release -- ingest --zip ./ReleasableAircraft.zip

# Current registration + ownership history (N-number or Mode S hex)
cargo run --release -- lookup N12345
cargo run --release -- lookup 12345
cargo run --release -- lookup A00B1C

# Search current owners (FTS5)
cargo run --release -- search-owner "BANK OF UTAH"

# Last ingest counts
cargo run --release -- status
```

`--db path/to/file.sqlite` is accepted on every command.

`--min-master-rows` (default 300000) aborts if `MASTER.txt` looks truncated.

`--faa-user-agent` / `FAA_USER_AGENT` override the Safari-like token required by `registry.faa.gov`. Do not send that UA to SEC.

## What is ingested

| File | Table | Strategy |
| --- | --- | --- |
| `MASTER.txt` | `aircraft` | SCD Type 2 on ownership/registration state |
| `ACFTREF.txt` | `aircraft_ref` | Upsert by `code` (captured dictionary; not a nightly delete-all) |
| `ENGINE.txt` | `engine_ref` | Upsert by `code` (captured dictionary; not a nightly delete-all) |
| `DEREG.txt` | `deregistered` | SCD Type 2 |
| `DOCINDEX.txt` | `documents` | Append-only unique rows (accumulates the FAA’s ~180-day window) |
| `DEALER.txt` | `dealers` | Full replace (dealer certificate roster) |
| `RESERVED.txt` | `reserved` | Full replace (current N-number reservations) |

## Identifiers

- `n_number` is stored with a leading `N` (`1WM` → `N1WM`).
- `icao24` is lowercase Mode S hex for joins with tail-to-ticker / the journal. Empty if the dump hex is empty.
- `mode_s_hex` is the raw dump field. Do not join on it.

## Parsing

The dump is documented as “comma-delimited,” but commas sit between fixed-width fields and also appear *inside* owner names, engine models, and collateral text. This tool slices by the 1-indexed byte positions in `ardata.pdf` and decodes Windows-1252. Short, empty-key, or garbage-date rows go to `parse_errors` instead of aborting the run.

## Schema notes

- WAL mode, `synchronous=NORMAL`, `busy_timeout=5000`, `foreign_keys=ON`
- One current row per N-number (`UNIQUE ... WHERE is_current = 1`)
- `state_hash` is BLAKE3 over the tracked columns
- `aircraft_fts` is an FTS5 virtual table (`content='aircraft'`) rebuilt each ingest. Query `aircraft_fts` (or `JOIN aircraft ON aircraft.id = aircraft_fts.rowid`). Do not query shadow tables.

## Dates

SQLite stores these as `TEXT`. Two shapes, no Unix epochs, no `DATETIME` column types:

| Kind | Format | Columns |
| --- | --- | --- |
| Date | `YYYY-MM-DD` | FAA calendar days and SCD `valid_from` / `valid_to` / `ingest_runs.as_of_date` |
| Instant | `YYYY-MM-DDTHH:MM:SSZ` | `ingest_runs.started_at` / `finished_at`, `documents.first_seen` |

The dump’s `YYYYMMDD` values are normalized at parse. Empty FAA dates stay empty. Garbage date fields quarantine the row. `year_mfr` is a year string, not a date.

After identifier or date-shape changes, wipe `data/faa-registry.sqlite` (+ `-wal`/`-shm`) and re-ingest. Same-day SCD versions stay distinct via `id` / `ingest_id`.
