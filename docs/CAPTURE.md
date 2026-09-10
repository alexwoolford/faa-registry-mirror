# Capture contract (mosaic vs sqlite)

Decision: **SCD history stays in work sqlite. Mosaic should see one current FAA row per `n_number`.** Do not treat a collector snapshot of this database as a normal ops move.

## What is true today

Work sqlite (`open_work`) is the source of truth for ownership history. `lookup` reads SCD2 `aircraft` / `deregistered` (`is_current`, `valid_from` / `valid_to`). Same-day versions are distinct via `id` / `ingest_id`.

Capture (`capturable-state` v0.1.0) keys `aircraft` by surrogate `id`. A registration change closes the old row (`UPDATE is_current=0` → outbox `U`) and inserts a new `id` (outbox `I`). `capture.current` therefore holds **every SCD version**, and `warehouse.aircraft` throws the closed ones away with `is_current = 1`.

A `--snapshot` of this announce name re-emits every captured table. That is how dictionary backfill worked once. Incremental dictionary upsert is the steady state. Snapshotting MASTER + deregistration history to refresh CESSNA names is the wrong tool.

## Capture set

| Table | Captured? | Why |
| --- | --- | --- |
| `ingest_runs` | after | Domain telemetry |
| `aircraft` | full, exclude `state_hash` | Mosaic `warehouse.aircraft` (filter current until the projection below exists) |
| `aircraft_ref` / `engine_ref` | after | Names; upsert must not nightly delete-all |
| `deregistered` / `documents` / `parse_errors` | no | No mosaic warehouse consumer; keep for local `lookup` / ops |
| `dealers` / `reserved` / `aircraft_fts` | no | Published sqlite extras / derived |

`open()` on `current/` must not install `_outbox`. Collector watches **work** sqlite only.

Stale mosaic rows for retired tables (`tbl` in `deregistered`, `documents`, `parse_errors`) are orphans. Incremental drain will not delete them. Do not snapshot to “fix” that. A one-shot `DELETE FROM capture.current WHERE src_db = 'faa-registry-mirror' AND tbl IN (...)` is mosaic hygiene if those rows bother you.

## Next schema change (not done here)

Ship mosaic **one row per tail**:

1. Keep SCD2 `aircraft` in sqlite, uncaptured or captured only for local tooling.
2. Capture a current projection keyed by **`n_number`** (`CaptureMode::After`): upsert on ingest when `is_current=1`, delete/retract when a tail leaves MASTER.
3. Point `warehouse.aircraft` at that table (or the same `tbl='aircraft'` once the key is `n_number` and closed rows are gone).

Until that ships, never snapshot this tile to refresh one dictionary, and do not grow the capture set “for completeness.”

## Sibling tiles

`tail-to-ticker` must read published `current/faa-registry.sqlite`, not a second GET of `ReleasableAircraft.zip`. The published file is the API. Do not extract a shared parse crate.
