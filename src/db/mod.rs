pub mod ingest;
pub mod query;

use std::ops::{Deref, DerefMut};
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Published `current/` and read-only lookup/status. Does not migrate STRICT
/// and does not install `_outbox` (that copy must not announce).
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
    }
    let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;
    conn.execute_batch(include_str!("schema.sql"))
        .context("apply schema")?;
    ensure_column(&conn, "ingest_runs", "dealer_rows", "INTEGER")?;
    ensure_column(&conn, "ingest_runs", "reserved_rows", "INTEGER")?;
    Ok(conn)
}

/// Writable work sqlite: STRICT migrate **before** capture triggers, then `_outbox`.
pub struct WorkDb {
    conn: Connection,
    pub nudge: crate::capture::Nudge,
}

impl Deref for WorkDb {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.conn
    }
}

impl DerefMut for WorkDb {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

pub fn open_work(path: &Path) -> Result<WorkDb> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
    }
    let conn = Connection::open(path).with_context(|| format!("open work {}", path.display()))?;
    crate::capture::apply_runtime_pragmas(&conn)?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.execute_batch(include_str!("schema.sql"))
        .context("apply schema")?;
    ensure_column(&conn, "ingest_runs", "dealer_rows", "INTEGER")?;
    ensure_column(&conn, "ingest_runs", "reserved_rows", "INTEGER")?;
    migrate_strict(&conn)?;
    conn.execute_batch(include_str!("schema.sql"))
        .context("reapply indexes after strict migrate")?;
    let nudge = install_capture(&conn, path)?;
    Ok(WorkDb { conn, nudge })
}

const FTS_REBUILD_SQL: &str = "
DROP TABLE IF EXISTS aircraft_fts;
CREATE VIRTUAL TABLE aircraft_fts USING fts5(
    n_number UNINDEXED,
    owner_name,
    city,
    state,
    content='aircraft',
    content_rowid='id'
);
INSERT INTO aircraft_fts (rowid, n_number, owner_name, city, state)
SELECT id, n_number, owner_name, city, state FROM aircraft WHERE is_current = 1;
";

pub(crate) fn rebuild_aircraft_fts(conn: &Connection) -> Result<()> {
    conn.execute_batch(FTS_REBUILD_SQL)
        .context("rebuild aircraft_fts")?;
    Ok(())
}

/// Snapshot `src` to `dest` with `VACUUM INTO`. Uses the process SQLite (bundled),
/// not the host `sqlite3` CLI (Oracle Linux 9 is 3.34 and cannot open STRICT).
/// Does not apply schema or install capture. `dest` must not already exist.
pub fn vacuum_into(src: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        anyhow::bail!(
            "VACUUM INTO destination already exists: {}",
            dest.display()
        );
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
    }
    let dest_str = dest
        .to_str()
        .with_context(|| format!("non-utf8 path {}", dest.display()))?;
    if dest_str.contains('\'') {
        anyhow::bail!("VACUUM INTO destination must not contain a single quote");
    }
    let conn = Connection::open(src).with_context(|| format!("open {}", src.display()))?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;
    conn.execute(&format!("VACUUM INTO '{dest_str}'"), [])
        .with_context(|| format!("VACUUM INTO {}", dest.display()))?;
    Ok(())
}

const DB_NAME: &str = "faa-registry-mirror";
const STATE_HASH: &[&str] = &["state_hash"];
const RAW_LINE: &[&str] = &["raw_line"];

fn install_capture(conn: &Connection, path: &Path) -> Result<crate::capture::Nudge> {
    let tables = [
        crate::capture::TableSpec::new("ingest_runs", crate::capture::CaptureMode::After),
        crate::capture::TableSpec::new("aircraft", crate::capture::CaptureMode::Full)
            .exclude(STATE_HASH),
        crate::capture::TableSpec::new("deregistered", crate::capture::CaptureMode::Full)
            .exclude(STATE_HASH),
        crate::capture::TableSpec::new("documents", crate::capture::CaptureMode::After),
        crate::capture::TableSpec::new("parse_errors", crate::capture::CaptureMode::After)
            .exclude(RAW_LINE),
    ];
    crate::capture::install(
        conn,
        &crate::capture::CaptureConfig::new(DB_NAME, path, &tables),
    )
}

fn migrate_strict(conn: &Connection) -> Result<()> {
    if crate::capture::table_is_strict(conn, "ingest_runs")? {
        return Ok(());
    }
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    conn.execute_batch(include_str!("migrate_strict.sql"))
        .context("migrate tables to STRICT")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    rebuild_aircraft_fts(conn)?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .with_context(|| format!("pragma table_info {table}"))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.as_deref() == Ok(column));
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )
        .with_context(|| format!("add {table}.{column}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn tmp_work() -> (std::path::PathBuf, WorkDb) {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "faa-cap-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = open_work(&dir.join("faa-registry.sqlite")).unwrap();
        (dir, db)
    }

    fn outbox_ops(db: &WorkDb) -> Vec<(String, String)> {
        let mut stmt = db
            .prepare("SELECT tbl, op FROM _outbox ORDER BY seq")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    fn outbox_count(db: &WorkDb) -> i64 {
        db.query_row("SELECT COUNT(*) FROM _outbox", [], |r| r.get(0))
            .unwrap()
    }

    fn insert_run(db: &WorkDb) -> i64 {
        db.execute(
            "INSERT INTO ingest_runs (as_of_date, started_at, source, status)
             VALUES ('2026-09-01', '2026-09-01T00:00:00Z', 'test', 'ok')",
            [],
        )
        .unwrap();
        db.last_insert_rowid()
    }

    fn insert_aircraft(db: &WorkDb, ingest_id: i64, owner: &str) {
        db.execute(
            "INSERT INTO aircraft (
                n_number, serial_number, mfr_mdl_code, eng_mfr_mdl, year_mfr, type_registrant,
                owner_name, street, street2, city, state, zip_code, region, county, country,
                last_action_date, cert_issue_date, certification, type_aircraft, type_engine,
                status_code, mode_s_code, fractional_owner, air_worth_date, other_names,
                expiration_date, unique_id, kit_mfr, kit_model, mode_s_hex, icao24, state_hash,
                valid_from, is_current, ingest_id
             ) VALUES (
                'N1WM','1','M','E','1998','3',
                ?1,'S','','DENVER','CO','80202','C','001','US',
                '2023-07-18','2019-02-13','','4','1',
                'V','','N','1998-04-30','',
                '2029-02-28','U','','','A00B1C','a00b1c','hash-secret',
                '2026-09-01',1,?2
             )",
            params![owner, ingest_id],
        )
        .unwrap();
    }

    #[test]
    fn published_open_does_not_install_capture() {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "faa-pub-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let conn = open(&dir.join("faa-registry.sqlite")).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = '_outbox'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn aircraft_insert_update_outbox_omits_state_hash() {
        let (dir, db) = tmp_work();
        let ingest_id = insert_run(&db);
        insert_aircraft(&db, ingest_id, "BANK OF UTAH TRUSTEE");
        db.execute(
            "UPDATE aircraft SET owner_name = 'NEW OWNER LLC' WHERE n_number = 'N1WM'",
            [],
        )
        .unwrap();
        let aircraft: Vec<String> = outbox_ops(&db)
            .into_iter()
            .filter(|(tbl, _)| tbl == "aircraft")
            .map(|(_, op)| op)
            .collect();
        assert_eq!(aircraft, vec!["I".to_string(), "U".to_string()]);
        let after: String = db
            .query_row(
                "SELECT after FROM _outbox WHERE tbl = 'aircraft' AND op = 'U'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !after.contains("state_hash"),
            "state_hash must not be in payload: {after}"
        );
        assert!(after.contains("NEW OWNER LLC"), "{after}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rollback_leaves_outbox_unchanged() {
        let (dir, mut db) = tmp_work();
        let ingest_id = insert_run(&db);
        let before = outbox_count(&db);
        {
            let tx = db.transaction().unwrap();
            tx.execute(
                "INSERT INTO aircraft (
                    n_number, serial_number, mfr_mdl_code, eng_mfr_mdl, year_mfr, type_registrant,
                    owner_name, street, street2, city, state, zip_code, region, county, country,
                    last_action_date, cert_issue_date, certification, type_aircraft, type_engine,
                    status_code, mode_s_code, fractional_owner, air_worth_date, other_names,
                    expiration_date, unique_id, kit_mfr, kit_model, mode_s_hex, icao24, state_hash,
                    valid_from, is_current, ingest_id
                 ) VALUES (
                    'N99','1','M','E','1998','3',
                    'X','S','','DENVER','CO','80202','C','001','US',
                    '2023-07-18','2019-02-13','','4','1',
                    'V','','N','1998-04-30','',
                    '2029-02-28','U','','','ABCDEF','abcdef','hash-secret',
                    '2026-09-01',1,?1
                 )",
                params![ingest_id],
            )
            .unwrap();
            tx.rollback().unwrap();
        }
        assert_eq!(outbox_count(&db), before);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn open_work_rebuilds_fts_after_strict_migrate() {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "faa-fts-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("faa-registry.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            let sql = include_str!("schema.sql").replace(") STRICT;", ");");
            conn.execute_batch(&sql).unwrap();
            assert!(!crate::capture::table_is_strict(&conn, "ingest_runs").unwrap());
            conn.execute(
                "INSERT INTO ingest_runs (as_of_date, started_at, source, status)
                 VALUES ('2026-09-01', '2026-09-01T00:00:00Z', 'test', 'ok')",
                [],
            )
            .unwrap();
            let ingest_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO aircraft (
                    n_number, serial_number, mfr_mdl_code, eng_mfr_mdl, year_mfr, type_registrant,
                    owner_name, street, street2, city, state, zip_code, region, county, country,
                    last_action_date, cert_issue_date, certification, type_aircraft, type_engine,
                    status_code, mode_s_code, fractional_owner, air_worth_date, other_names,
                    expiration_date, unique_id, kit_mfr, kit_model, mode_s_hex, icao24, state_hash,
                    valid_from, is_current, ingest_id
                 ) VALUES (
                    'N1WM','1','M','E','1998','3',
                    'BANK OF UTAH TRUSTEE','S','','DENVER','CO','80202','C','001','US',
                    '2023-07-18','2019-02-13','','4','1',
                    'V','','N','1998-04-30','',
                    '2029-02-28','U','','','A00B1C','a00b1c','hash-secret',
                    '2026-09-01',1,?1
                 )",
                params![ingest_id],
            )
            .unwrap();
            let hits: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM aircraft_fts WHERE aircraft_fts MATCH 'BANK'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(hits, 0, "pre-migrate FTS index must be empty so rebuild is visible");
        }
        let db = open_work(&path).unwrap();
        assert!(crate::capture::table_is_strict(&db, "ingest_runs").unwrap());
        let n_number: String = db
            .query_row(
                "SELECT n_number FROM aircraft_fts WHERE aircraft_fts MATCH 'BANK'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n_number, "N1WM");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn vacuum_into_copies_without_installing_capture_on_open() {
        let (dir, db) = tmp_work();
        let ingest_id = insert_run(&db);
        insert_aircraft(&db, ingest_id, "BANK OF UTAH TRUSTEE");
        drop(db);
        let src = dir.join("faa-registry.sqlite");
        let dest = dir.join("snapshot.sqlite");
        vacuum_into(&src, &dest).unwrap();
        assert!(dest.exists());
        let err = vacuum_into(&src, &dest).unwrap_err();
        assert!(
            err.to_string().contains("already exists"),
            "{err}"
        );
        let snap = Connection::open(&dest).unwrap();
        let n: i64 = snap
            .query_row("SELECT COUNT(*) FROM aircraft WHERE is_current = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
