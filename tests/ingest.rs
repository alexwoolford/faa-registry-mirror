use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use faa_registry_mirror::db::ingest::{ingest, IngestOptions};
use faa_registry_mirror::db::query;
use faa_registry_mirror::download::{write_test_zip, FAA_DOWNLOAD_USER_AGENT};
use faa_registry_mirror::parse::fixed_width::{padded_record, write_field};

fn master_line(n: &str, owner: &str, serial: &str, state: &str) -> Vec<u8> {
    let mut buf = padded_record(612);
    write_field(&mut buf, 1, n);
    write_field(&mut buf, 7, serial);
    write_field(&mut buf, 38, "2072703");
    write_field(&mut buf, 46, "41530");
    write_field(&mut buf, 52, "1998");
    write_field(&mut buf, 57, "3");
    write_field(&mut buf, 59, owner);
    write_field(&mut buf, 110, "123 MAIN ST");
    write_field(&mut buf, 178, "DENVER");
    write_field(&mut buf, 197, state);
    write_field(&mut buf, 200, "80202");
    write_field(&mut buf, 217, "US");
    write_field(&mut buf, 220, "20230718");
    write_field(&mut buf, 229, "20190213");
    write_field(&mut buf, 249, "4");
    write_field(&mut buf, 251, "1");
    write_field(&mut buf, 254, "V");
    write_field(&mut buf, 268, "19980430");
    write_field(&mut buf, 532, "20290228");
    write_field(&mut buf, 602, "A00B1C");
    buf
}

fn acftref_line() -> Vec<u8> {
    let mut buf = padded_record(158);
    write_field(&mut buf, 1, "2072703");
    write_field(&mut buf, 9, "CESSNA");
    write_field(&mut buf, 40, "172S");
    write_field(&mut buf, 61, "4");
    write_field(&mut buf, 63, "1");
    write_field(&mut buf, 73, "4");
    buf
}

fn engine_line() -> Vec<u8> {
    let mut buf = padded_record(47);
    write_field(&mut buf, 1, "41530");
    write_field(&mut buf, 7, "LYCOMING");
    write_field(&mut buf, 18, "IO-360-L2A");
    write_field(&mut buf, 32, "1");
    write_field(&mut buf, 35, "180");
    buf
}

fn dereg_line() -> Vec<u8> {
    let mut buf = padded_record(723);
    write_field(&mut buf, 1, "55555");
    write_field(&mut buf, 7, "SN-DREG");
    write_field(&mut buf, 38, "2072703");
    write_field(&mut buf, 46, "6");
    write_field(&mut buf, 49, "OLD OWNER LLC");
    write_field(&mut buf, 241, "20240115");
    write_field(&mut buf, 261, "CANADA");
    buf
}

fn docindex_line() -> Vec<u8> {
    let mut buf = padded_record(180);
    write_field(&mut buf, 1, "1");
    write_field(&mut buf, 3, "12345");
    write_field(&mut buf, 41, "BANK, N.A.");
    write_field(&mut buf, 92, "DOC000111222");
    write_field(&mut buf, 105, "20240601");
    write_field(&mut buf, 165, "SECURITY");
    buf
}

fn dealer_line() -> Vec<u8> {
    let mut buf = padded_record(1465);
    write_field(&mut buf, 1, "26-0001");
    write_field(&mut buf, 9, "7");
    write_field(&mut buf, 11, "20230101");
    write_field(&mut buf, 20, "20251231");
    write_field(&mut buf, 36, "CESSNA AIRCRAFT CO");
    write_field(&mut buf, 155, "WICHITA");
    write_field(&mut buf, 174, "KS");
    buf
}

fn reserved_line() -> Vec<u8> {
    let mut buf = padded_record(194);
    write_field(&mut buf, 1, "1WM");
    write_field(&mut buf, 7, "META PLATFORMS INC");
    write_field(&mut buf, 126, "MENLO PARK");
    write_field(&mut buf, 145, "CA");
    write_field(&mut buf, 159, "20240115");
    write_field(&mut buf, 168, "FP");
    write_field(&mut buf, 186, "20270305");
    buf
}

fn join_lines(lines: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for line in lines {
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    out
}

fn build_zip(master_lines: &[Vec<u8>]) -> Vec<u8> {
    build_zip_refs(master_lines, &[acftref_line()], &[engine_line()])
}

fn build_zip_refs(master_lines: &[Vec<u8>], acftref: &[Vec<u8>], engine: &[Vec<u8>]) -> Vec<u8> {
    write_test_zip(&[
        ("MASTER.txt", &join_lines(master_lines)),
        ("ACFTREF.txt", &join_lines(acftref)),
        ("ENGINE.txt", &join_lines(engine)),
        ("DEREG.txt", &join_lines(&[dereg_line()])),
        ("DOCINDEX.txt", &join_lines(&[docindex_line()])),
        ("DEALER.txt", &join_lines(&[dealer_line()])),
        ("RESERVED.txt", &join_lines(&[reserved_line()])),
    ])
    .expect("zip")
}

fn acftref_extra() -> Vec<u8> {
    let mut buf = padded_record(158);
    write_field(&mut buf, 1, "7100510");
    write_field(&mut buf, 9, "PIPER");
    write_field(&mut buf, 40, "J3C-65");
    write_field(&mut buf, 61, "4");
    write_field(&mut buf, 63, "1");
    write_field(&mut buf, 73, "2");
    buf
}

fn engine_rotax() -> Vec<u8> {
    let mut buf = padded_record(47);
    write_field(&mut buf, 1, "55593");
    write_field(&mut buf, 7, "ROTAX");
    write_field(&mut buf, 18, "912 IS");
    write_field(&mut buf, 32, "7");
    write_field(&mut buf, 35, "100");
    buf
}

fn outbox_ops_after(conn: &rusqlite::Connection, tbl: &str, min_seq: i64) -> (i64, i64, i64) {
    let count = |op: &str| -> i64 {
        conn.query_row(
            "SELECT count(*) FROM _outbox WHERE tbl = ?1 AND op = ?2 AND seq > ?3",
            rusqlite::params![tbl, op, min_seq],
            |row| row.get(0),
        )
        .unwrap()
    };
    (count("I"), count("U"), count("D"))
}

fn max_outbox_seq(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM _outbox", [], |row| {
        row.get(0)
    })
    .unwrap()
}

fn temp_paths(label: &str) -> (PathBuf, PathBuf) {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "faa-registry-{}-{}-{}",
        label,
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("registry.sqlite"), dir.join("dump.zip"))
}

fn run_ingest(
    db: &Path,
    zip: &Path,
    bytes: &[u8],
    force: bool,
) -> faa_registry_mirror::db::ingest::IngestStats {
    std::fs::write(zip, bytes).unwrap();
    ingest(&IngestOptions {
        db_path: db.to_path_buf(),
        zip_path: Some(zip.to_path_buf()),
        cache_dir: None,
        zip_url: String::new(),
        min_master_rows: 1,
        force,
        user_agent: FAA_DOWNLOAD_USER_AGENT.to_string(),
    })
    .expect("ingest")
}

#[test]
fn ingest_lookup_history_search_and_status() {
    let (db, zip) = temp_paths("happy");
    let first = build_zip(&[
        master_line("12345", "BANK OF UTAH TRUSTEE", "SN1", "CO"),
        master_line("99ABC", "SMITH JOHN", "SN2", "TX"),
    ]);
    let stats = run_ingest(&db, &zip, &first, false);
    assert_eq!(stats.new_rows, 2);
    assert_eq!(stats.changed_rows, 0);
    assert_eq!(stats.documents_inserted, 1);
    assert_eq!(stats.dereg_new, 1);
    assert_eq!(stats.dereg_changed, 0);
    assert_eq!(stats.dereg_closed, 0);
    assert_eq!(stats.dealer_rows, 1);
    assert_eq!(stats.reserved_rows, 1);

    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let looked = query::lookup(&conn, "N12345").unwrap();
    let current = looked.current.expect("current row");
    assert_eq!(current.n_number, "N12345");
    assert_eq!(current.icao24, "a00b1c");
    assert_eq!(current.owner_name, "BANK OF UTAH TRUSTEE");
    assert_eq!(current.aircraft_mfr.as_deref(), Some("CESSNA"));
    assert_eq!(current.aircraft_model.as_deref(), Some("172S"));
    assert_eq!(current.engine_mfr.as_deref(), Some("LYCOMING"));
    assert_eq!(current.cert_issue_date, "2019-02-13");
    assert_eq!(current.expiration_date, "2029-02-28");
    assert_eq!(current.valid_from.len(), 10);
    assert_eq!(&current.valid_from[4..5], "-");
    assert!(looked.history.is_empty());
    assert_eq!(looked.documents.len(), 1);
    assert_eq!(looked.documents[0].doc_type, "SECURITY");
    assert_eq!(looked.documents[0].receipt_date, "2024-06-01");

    let (dealer_name, reserved_n, reserved_who): (String, String, String) = conn
        .query_row(
            "SELECT d.name, r.n_number, r.registrant FROM dealers d, reserved r",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(dealer_name, "CESSNA AIRCRAFT CO");
    assert_eq!(reserved_n, "N1WM");
    assert_eq!(reserved_who, "META PLATFORMS INC");

    let dereg = query::lookup(&conn, "55555").unwrap();
    assert_eq!(dereg.deregistered[0].cancel_date, "2024-01-15");
    assert_eq!(dereg.deregistered[0].valid_from.len(), 10);

    let fts_shadows: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE name LIKE 'aircraft_fts%' ORDER BY 1")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert!(fts_shadows.contains(&"aircraft_fts".into()));
    assert!(
        !fts_shadows.iter().any(|n| n == "aircraft_fts_content"),
        "external-content FTS should not create c0–c3 content table: {fts_shadows:?}"
    );

    let (as_of, started): (String, String) = conn
        .query_row(
            "SELECT as_of_date, started_at FROM ingest_runs ORDER BY id LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(as_of.len(), 10);
    assert_eq!(&as_of[4..5], "-");
    assert!(started.ends_with('Z'), "{started}");
    assert!(!started.contains("+00:00"), "{started}");

    let by_hex = query::lookup(&conn, "A00B1C").unwrap();
    assert_eq!(by_hex.current.unwrap().n_number, "N12345");
    let by_bare = query::lookup(&conn, "12345").unwrap();
    assert_eq!(by_bare.current.unwrap().n_number, "N12345");

    let skipped = run_ingest(&db, &zip, &first, false);
    assert!(skipped.skipped_same_zip);

    let again = run_ingest(&db, &zip, &first, true);
    assert!(!again.skipped_same_zip);
    assert_eq!(again.new_rows, 0);
    assert_eq!(again.changed_rows, 0);
    assert_eq!(again.unchanged_rows, 2);
    assert_eq!(again.documents_inserted, 0);

    let second = build_zip(&[
        master_line("12345", "NEW OWNER LLC", "SN1", "CO"),
        master_line("99ABC", "SMITH JOHN", "SN2", "TX"),
    ]);
    let changed = run_ingest(&db, &zip, &second, false);
    assert_eq!(changed.changed_rows, 1);
    assert_eq!(changed.unchanged_rows, 1);

    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let looked = query::lookup(&conn, "12345").unwrap();
    assert_eq!(looked.current.unwrap().owner_name, "NEW OWNER LLC");
    assert_eq!(looked.history.len(), 1);
    assert_eq!(looked.history[0].owner_name, "BANK OF UTAH TRUSTEE");
    let closed_on = looked.history[0].valid_to.as_deref().expect("valid_to");
    assert_eq!(closed_on.len(), 10);
    assert_eq!(&closed_on[4..5], "-");

    let hits = query::search_owner(&conn, "NEW OWNER", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].n_number, "N12345");

    let status = query::latest_status(&conn).unwrap().expect("status");
    assert_eq!(status.status, "ok");
    assert_eq!(status.changed_rows, Some(1));
    assert_eq!(status.dereg_new, Some(0));
    assert_eq!(status.dereg_changed, Some(0));
    assert_eq!(status.dereg_closed, Some(0));
    assert_eq!(status.dealer_rows, Some(1));
    assert_eq!(status.reserved_rows, Some(1));

    let dropped = build_zip(&[master_line("12345", "NEW OWNER LLC", "SN1", "CO")]);
    let closed = run_ingest(&db, &zip, &dropped, false);
    assert_eq!(closed.closed_rows, 1);
    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let gone = query::lookup(&conn, "99ABC").unwrap();
    assert!(gone.current.is_none());
    assert_eq!(gone.history.len(), 1);

    let _ = std::fs::remove_dir_all(db.parent().unwrap());
}

#[test]
fn refuses_truncated_master() {
    let (db, zip) = temp_paths("trunc");
    let bytes = write_test_zip(&[
        ("MASTER.txt", b""),
        ("ACFTREF.txt", b""),
        ("ENGINE.txt", b""),
        ("DEREG.txt", b""),
        ("DOCINDEX.txt", b""),
        ("DEALER.txt", b""),
        ("RESERVED.txt", b""),
    ])
    .unwrap();
    std::fs::write(&zip, bytes).unwrap();
    let err = ingest(&IngestOptions {
        db_path: db.clone(),
        zip_path: Some(zip.clone()),
        min_master_rows: 300_000,
        ..IngestOptions::default()
    })
    .unwrap_err();
    assert!(err.to_string().contains("expected at least"));

    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let aircraft: i64 = conn
        .query_row("SELECT count(*) FROM aircraft", [], |row| row.get(0))
        .unwrap();
    assert_eq!(aircraft, 0);
    let status = query::latest_status(&conn).unwrap().expect("failed run");
    assert_eq!(status.status, "failed");
    let _ = std::fs::remove_dir_all(db.parent().unwrap());
}

#[test]
fn dictionary_upsert_does_not_rewrite_unchanged_codes() {
    let (db, zip) = temp_paths("dicts");
    let masters = [master_line("12345", "BANK OF UTAH TRUSTEE", "SN1", "CO")];
    let first = build_zip_refs(
        &masters,
        &[acftref_line(), acftref_extra()],
        &[engine_line(), engine_rotax()],
    );
    run_ingest(&db, &zip, &first, false);

    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let n_ac: i64 = conn
        .query_row("SELECT count(*) FROM aircraft_ref", [], |row| row.get(0))
        .unwrap();
    let n_eng: i64 = conn
        .query_row("SELECT count(*) FROM engine_ref", [], |row| row.get(0))
        .unwrap();
    assert_eq!(n_ac, 2);
    assert_eq!(n_eng, 2);
    let (i, u, d) = outbox_ops_after(&conn, "aircraft_ref", 0);
    assert_eq!((i, u, d), (2, 0, 0));
    let (i, u, d) = outbox_ops_after(&conn, "engine_ref", 0);
    assert_eq!((i, u, d), (2, 0, 0));
    let seq_after_first = max_outbox_seq(&conn);
    drop(conn);

    let again = build_zip_refs(
        &masters,
        &[acftref_line(), acftref_extra()],
        &[engine_line(), engine_rotax()],
    );
    run_ingest(&db, &zip, &again, true);
    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let cessna: String = conn
        .query_row(
            "SELECT mfr FROM aircraft_ref WHERE code = '2072703'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cessna, "CESSNA");
    assert_eq!(
        outbox_ops_after(&conn, "aircraft_ref", seq_after_first),
        (0, 0, 0),
        "unchanged ACFTREF must not delete/reinsert"
    );
    assert_eq!(
        outbox_ops_after(&conn, "engine_ref", seq_after_first),
        (0, 0, 0),
        "unchanged ENGINE must not delete/reinsert"
    );
    let seq_after_same = max_outbox_seq(&conn);
    drop(conn);

    let dropped = build_zip_refs(&masters, &[acftref_line()], &[engine_line()]);
    run_ingest(&db, &zip, &dropped, true);
    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let n_ac: i64 = conn
        .query_row("SELECT count(*) FROM aircraft_ref", [], |row| row.get(0))
        .unwrap();
    let n_eng: i64 = conn
        .query_row("SELECT count(*) FROM engine_ref", [], |row| row.get(0))
        .unwrap();
    assert_eq!(n_ac, 1);
    assert_eq!(n_eng, 1);
    assert_eq!(
        outbox_ops_after(&conn, "aircraft_ref", seq_after_same),
        (0, 0, 1)
    );
    assert_eq!(
        outbox_ops_after(&conn, "engine_ref", seq_after_same),
        (0, 0, 1)
    );

    let _ = std::fs::remove_dir_all(db.parent().unwrap());
}

#[test]
fn duplicate_master_n_number_goes_to_parse_errors() {
    let (db, zip) = temp_paths("dupn");
    let first = build_zip(&[
        master_line("12345", "BANK OF UTAH TRUSTEE", "SN1", "CO"),
        master_line("12345", "OTHER OWNER", "SN9", "TX"),
    ]);
    let stats = run_ingest(&db, &zip, &first, false);
    assert_eq!(stats.new_rows, 1);
    assert_eq!(stats.skipped_rows, 1);
    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT count(*) FROM aircraft WHERE is_current = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
    let owner: String = conn
        .query_row(
            "SELECT owner_name FROM aircraft WHERE is_current = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(owner, "BANK OF UTAH TRUSTEE");
    let err: String = conn
        .query_row(
            "SELECT error FROM parse_errors WHERE file_name = 'MASTER.txt'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(err.contains("duplicate N-number"), "{err}");
    let _ = std::fs::remove_dir_all(db.parent().unwrap());
}

#[test]
fn master_change_emits_close_u_and_insert_i() {
    let (db, zip) = temp_paths("scdcap");
    let first = build_zip(&[master_line("12345", "BANK OF UTAH TRUSTEE", "SN1", "CO")]);
    run_ingest(&db, &zip, &first, false);
    let conn = faa_registry_mirror::db::open(&db).unwrap();
    let seq = max_outbox_seq(&conn);
    drop(conn);

    let second = build_zip(&[master_line("12345", "NEW OWNER LLC", "SN1", "CO")]);
    run_ingest(&db, &zip, &second, false);
    let conn = faa_registry_mirror::db::open(&db).unwrap();
    assert_eq!(outbox_ops_after(&conn, "aircraft", seq), (1, 1, 0));
    let after_u: String = conn
        .query_row(
            "SELECT after FROM _outbox WHERE tbl = 'aircraft' AND op = 'U' AND seq > ?1",
            rusqlite::params![seq],
            |row| row.get(0),
        )
        .unwrap();
    assert!(after_u.contains("\"is_current\":0") || after_u.contains("\"is_current\": 0"), "{after_u}");
    let _ = std::fs::remove_dir_all(db.parent().unwrap());
}

#[test]
fn local_only_tables_do_not_emit_outbox() {
    let (db, zip) = temp_paths("localonly");
    run_ingest(
        &db,
        &zip,
        &build_zip(&[master_line("12345", "BANK OF UTAH TRUSTEE", "SN1", "CO")]),
        false,
    );
    let conn = faa_registry_mirror::db::open(&db).unwrap();
    for tbl in [
        "deregistered",
        "documents",
        "parse_errors",
        "dealers",
        "reserved",
    ] {
        assert_eq!(outbox_ops_after(&conn, tbl, 0), (0, 0, 0), "{tbl}");
    }
    let _ = std::fs::remove_dir_all(db.parent().unwrap());
}
