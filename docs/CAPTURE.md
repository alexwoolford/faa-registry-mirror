# Capture contract (work sqlite)

Decision: **SCD history stays in work sqlite. Mosaic should see one current FAA row per `n_number`.** Capture the ownership trickle (current MASTER + dictionary names + ingest telemetry), not DEREG/DOCINDEX/dealer/reserved hose and not a collector snapshot of this database.

Work sqlite: `{--db}` (prod `/var/lib/faa-registry-mirror/work/faa-registry.sqlite`). Logical name: `faa-registry-mirror`. Pin: `capturable-state` git tag `v0.1.1` (not a path dep; do not copy `src/*.rs`).

Canonical contract: [capturable-state design principles](https://github.com/alexwoolford/capturable-state/blob/main/docs/design-principles.md) and [datetime.md](https://github.com/alexwoolford/capturable-state/blob/main/docs/datetime.md).

## What is true today

Work sqlite (`open_work`) is the source of truth for ownership history. `lookup` reads SCD2 `aircraft` / `deregistered` (`is_current`, `valid_from` / `valid_to`). Same-day versions are distinct via `id` / `ingest_id`.

Capture (`capturable-state` v0.1.1) keys `aircraft` by surrogate `id`. A registration change closes the old row (`UPDATE is_current=0` → outbox `U`) and inserts a new `id` (outbox `I`). `capture.current` therefore holds **every SCD version**, and `warehouse.aircraft` throws the closed ones away with `is_current = 1`.

A `--snapshot` of this announce name re-emits every captured table. That is how dictionary backfill worked once. Incremental dictionary upsert is the steady state. Snapshotting MASTER + deregistration history to refresh CESSNA names is the wrong tool.

Do not `collect --snapshot` this database.

## What is captured

| Table | Mode | Why |
| --- | --- | --- |
| `ingest_runs` | after | Domain telemetry |
| `aircraft` | full, exclude `state_hash` | Mosaic `warehouse.aircraft` (filter current until the projection below exists) |
| `aircraft_ref` / `engine_ref` | after | Names; upsert must not nightly delete-all |

Uncaptured: `deregistered`, `documents`, `parse_errors` (local `lookup` / ops; no warehouse consumer), `dealers` / `reserved` (published sqlite extras), `aircraft_fts` (derived). Do not grow the capture set “for completeness.”

`open()` on `current/` must not install `_outbox`. Collector watches **work** sqlite only.

Stale mosaic rows for retired tables (`tbl` in `deregistered`, `documents`, `parse_errors`) are orphans. Incremental drain will not delete them. Do not snapshot to “fix” that. A one-shot `DELETE FROM capture.current WHERE src_db = 'faa-registry-mirror' AND tbl IN (...)` is mosaic hygiene if those rows bother you.

## Identity

Grain: captured `aircraft` is keyed by surrogate `id`. Local uniqueness is one current row per `n_number` (`UNIQUE ... WHERE is_current = 1`). Dictionaries upsert on `code`. Soft-delete unused; SCD close is `is_current=0` (a `U`), not `deleted_at`.

## Clocks

| Layer | Columns | Type |
| --- | --- | --- |
| Facts | FAA calendar days; SCD `valid_from` / `valid_to`; `ingest_runs.as_of_date` | TEXT `YYYY-MM-DD` |
| Facts | `ingest_runs.started_at` / `finished_at`; `documents.first_seen` | TEXT `YYYY-MM-DDTHH:MM:SSZ` |
| Envelope | `_outbox.ts`, `deleted_at` | INTEGER Unix seconds |

Order outbox by `seq`, not `ts`.

## Announce / nudge

`install()` on work sqlite. Announce file stem is `faa-registry-mirror` (the logical name), not the sqlite file stem `faa-registry`. `ReadWritePaths` include `/var/lib/state-capture/announce` (required when the collector is present) and `-/run/state`. `faa` must be in group `state-capture`. Collector host inventory lives in mosaic `deploy/ct-firehose/`, not this crate.

## Next schema change (not done here)

Ship mosaic **one row per tail**:

1. Keep SCD2 `aircraft` in sqlite, uncaptured or captured only for local tooling.
2. Capture a current projection keyed by **`n_number`** (`CaptureMode::After`): upsert on ingest when `is_current=1`, delete/retract when a tail leaves MASTER.
3. Point `warehouse.aircraft` at that table (or the same `tbl='aircraft'` once the key is `n_number` and closed rows are gone).

Until that ships, never snapshot this tile to refresh one dictionary.

## Sibling tiles

`tail-to-ticker` must read published `current/faa-registry.sqlite`, not a second GET of `ReleasableAircraft.zip`. The published file is the API. Do not extract a shared parse crate.
