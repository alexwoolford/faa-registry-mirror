use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, Transaction};

use crate::canonical_icao24;
use crate::dates::{require_utc_date, require_utc_instant, utc_date, utc_iso};
use crate::download::{self, DEFAULT_ZIP_URL};
use crate::model::{
    AircraftRef, DealerRecord, DeregRecord, DocumentRecord, EngineRef, MasterRecord, ParseError,
    ReservedRecord,
};
use crate::parse::{
    parse_acftref, parse_dealer, parse_dereg, parse_docindex, parse_engine, parse_master,
    parse_reserved,
};

pub const DEFAULT_MIN_MASTER_ROWS: usize = 300_000;

#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub db_path: std::path::PathBuf,
    pub zip_path: Option<std::path::PathBuf>,
    pub cache_dir: Option<std::path::PathBuf>,
    pub zip_url: String,
    pub min_master_rows: usize,
    pub force: bool,
    pub user_agent: String,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            db_path: std::path::PathBuf::from("data/faa-registry.sqlite"),
            zip_path: None,
            cache_dir: None,
            zip_url: DEFAULT_ZIP_URL.to_string(),
            min_master_rows: DEFAULT_MIN_MASTER_ROWS,
            force: false,
            user_agent: crate::download::FAA_DOWNLOAD_USER_AGENT.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IngestStats {
    pub source: String,
    pub zip_hash: String,
    pub master_rows: usize,
    pub new_rows: usize,
    pub changed_rows: usize,
    pub closed_rows: usize,
    pub unchanged_rows: usize,
    pub skipped_rows: usize,
    pub dereg_new: usize,
    pub dereg_changed: usize,
    pub dereg_closed: usize,
    pub documents_inserted: usize,
    pub dealer_rows: usize,
    pub reserved_rows: usize,
    pub skipped_same_zip: bool,
}

pub fn ingest(opts: &IngestOptions) -> Result<IngestStats> {
    let (zip_bytes, source) = if let Some(path) = &opts.zip_path {
        (
            download::read_zip_file(path)?,
            format!("file:{}", path.display()),
        )
    } else {
        (
            download::download_zip(&opts.zip_url, &opts.user_agent)?,
            opts.zip_url.clone(),
        )
    };
    if opts.zip_path.is_none() {
        if let Some(dir) = &opts.cache_dir {
            if let Err(error) = download::write_zip_cache(dir, &zip_bytes) {
                tracing::warn!(error = %error, cache = %dir.display(), "failed to write zip cache");
            }
        }
    }
    let zip_hash = download::zip_hash(&zip_bytes);
    let as_of = utc_date();
    require_utc_date(&as_of, "as_of_date")?;
    let started_at = utc_iso(chrono::Utc::now());
    require_utc_instant(&started_at, "started_at")?;

    let mut work = crate::db::open_work(&opts.db_path)?;
    if !opts.force {
        if let Some(prev) = last_ok_zip_hash(&work)? {
            if prev == zip_hash {
                record_skipped_run(&work, &as_of, &started_at, &source, &zip_hash)?;
                work.nudge.send();
                tracing::info!(zip_hash = %zip_hash, "same zip_hash as last ok run; skipping SCD");
                return Ok(IngestStats {
                    source,
                    zip_hash,
                    skipped_same_zip: true,
                    ..IngestStats::default()
                });
            }
        }
    }

    let master_bytes = download::extract_named(&zip_bytes, "MASTER.txt")?;
    let acftref_bytes = download::extract_named(&zip_bytes, "ACFTREF.txt")?;
    let engine_bytes = download::extract_named(&zip_bytes, "ENGINE.txt")?;
    let dereg_bytes = download::extract_named(&zip_bytes, "DEREG.txt")?;
    let docindex_bytes = download::extract_named(&zip_bytes, "DOCINDEX.txt")?;
    let dealer_bytes = download::extract_named(&zip_bytes, "DEALER.txt")?;
    let reserved_bytes = download::extract_named(&zip_bytes, "RESERVED.txt")?;

    let master = parse_master(&master_bytes);
    let acftref = parse_acftref(&acftref_bytes);
    let engine = parse_engine(&engine_bytes);
    let dereg = parse_dereg(&dereg_bytes);
    let documents = parse_docindex(&docindex_bytes);
    let dealers = parse_dealer(&dealer_bytes);
    let reserved = parse_reserved(&reserved_bytes);

    let mut parse_errors = Vec::new();
    parse_errors.extend(master.errors);
    parse_errors.extend(acftref.errors);
    parse_errors.extend(engine.errors);
    parse_errors.extend(dereg.errors);
    parse_errors.extend(documents.errors);
    parse_errors.extend(dealers.errors);
    parse_errors.extend(reserved.errors);

    if master.records.len() < opts.min_master_rows {
        record_failed_run(
            &work,
            &as_of,
            &started_at,
            &source,
            &zip_hash,
            master.records.len(),
            parse_errors.len(),
            &format!(
                "MASTER.txt has {} data rows; refusing ingest below floor of {}",
                master.records.len(),
                opts.min_master_rows
            ),
        )?;
        work.nudge.send();
        bail!(
            "MASTER.txt has {} data rows; expected at least {} (truncated dump?)",
            master.records.len(),
            opts.min_master_rows
        );
    }

    let tx = work.transaction().context("begin ingest transaction")?;
    let ingest_id = insert_run_start(&tx, &as_of, &started_at, &source, &zip_hash)?;

    upsert_aircraft_ref(&tx, &acftref.records)?;
    upsert_engine_ref(&tx, &engine.records)?;
    replace_dealers(&tx, ingest_id, &dealers.records)?;
    replace_reserved(&tx, ingest_id, &reserved.records)?;

    let mut stats = IngestStats {
        source: source.clone(),
        zip_hash: zip_hash.clone(),
        master_rows: master.records.len(),
        skipped_rows: parse_errors.len(),
        dealer_rows: dealers.records.len(),
        reserved_rows: reserved.records.len(),
        ..IngestStats::default()
    };

    apply_master_scd(&tx, ingest_id, &master.records, &mut stats)?;
    apply_dereg_scd(&tx, ingest_id, &dereg.records, &mut stats)?;
    stats.documents_inserted = insert_documents(&tx, ingest_id, &documents.records)?;
    insert_parse_errors(&tx, ingest_id, &parse_errors)?;
    rebuild_fts(&tx)?;
    finish_run(&tx, ingest_id, &stats)?;
    tx.commit().context("commit ingest")?;
    work.nudge.send();

    tracing::info!(
        master = stats.master_rows,
        new = stats.new_rows,
        changed = stats.changed_rows,
        closed = stats.closed_rows,
        unchanged = stats.unchanged_rows,
        docs = stats.documents_inserted,
        dealers = stats.dealer_rows,
        reserved = stats.reserved_rows,
        "ingest complete"
    );
    Ok(stats)
}

fn last_ok_zip_hash(conn: &Connection) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT zip_hash FROM ingest_runs
         WHERE status = 'ok' AND zip_hash IS NOT NULL AND zip_hash != ''
         ORDER BY id DESC LIMIT 1",
    )?;
    let mut rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(hash) => Ok(Some(hash?)),
        None => Ok(None),
    }
}

fn record_skipped_run(
    conn: &Connection,
    as_of: &str,
    started_at: &str,
    source: &str,
    zip_hash: &str,
) -> Result<()> {
    let finished = utc_iso(chrono::Utc::now());
    require_utc_instant(&finished, "finished_at")?;
    conn.execute(
        "INSERT INTO ingest_runs (
            as_of_date, started_at, finished_at, source, zip_hash, status
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'skipped')",
        params![as_of, started_at, finished, source, zip_hash],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_failed_run(
    conn: &Connection,
    as_of: &str,
    started_at: &str,
    source: &str,
    zip_hash: &str,
    master_rows: usize,
    skipped: usize,
    error: &str,
) -> Result<()> {
    let finished = utc_iso(chrono::Utc::now());
    require_utc_instant(&finished, "finished_at")?;
    conn.execute(
        "INSERT INTO ingest_runs (
            as_of_date, started_at, finished_at, source, zip_hash, master_rows,
            skipped_rows, status, error
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'failed', ?8)",
        params![
            as_of,
            started_at,
            finished,
            source,
            zip_hash,
            master_rows as i64,
            skipped as i64,
            error
        ],
    )?;
    Ok(())
}

fn insert_run_start(
    tx: &Transaction<'_>,
    as_of: &str,
    started_at: &str,
    source: &str,
    zip_hash: &str,
) -> Result<i64> {
    tx.execute(
        "INSERT INTO ingest_runs (as_of_date, started_at, source, zip_hash, status)
         VALUES (?1, ?2, ?3, ?4, 'running')",
        params![as_of, started_at, source, zip_hash],
    )?;
    Ok(tx.last_insert_rowid())
}

fn finish_run(tx: &Transaction<'_>, ingest_id: i64, stats: &IngestStats) -> Result<()> {
    tx.execute(
        "UPDATE ingest_runs SET
            finished_at = ?1,
            master_rows = ?2,
            new_rows = ?3,
            changed_rows = ?4,
            closed_rows = ?5,
            unchanged_rows = ?6,
            skipped_rows = ?7,
            dereg_new = ?8,
            dereg_changed = ?9,
            dereg_closed = ?10,
            documents_inserted = ?11,
            dealer_rows = ?12,
            reserved_rows = ?13,
            status = 'ok'
         WHERE id = ?14",
        params![
            {
                let finished = utc_iso(chrono::Utc::now());
                require_utc_instant(&finished, "finished_at")?;
                finished
            },
            stats.master_rows as i64,
            stats.new_rows as i64,
            stats.changed_rows as i64,
            stats.closed_rows as i64,
            stats.unchanged_rows as i64,
            stats.skipped_rows as i64,
            stats.dereg_new as i64,
            stats.dereg_changed as i64,
            stats.dereg_closed as i64,
            stats.documents_inserted as i64,
            stats.dealer_rows as i64,
            stats.reserved_rows as i64,
            ingest_id,
        ],
    )?;
    Ok(())
}

fn upsert_aircraft_ref(tx: &Transaction<'_>, rows: &[AircraftRef]) -> Result<()> {
    tx.execute_batch("CREATE TEMP TABLE aircraft_ref_in (code TEXT PRIMARY KEY)")?;
    {
        let mut keep = tx.prepare("INSERT OR IGNORE INTO aircraft_ref_in (code) VALUES (?1)")?;
        let mut stmt = tx.prepare(
            "INSERT INTO aircraft_ref (
                code, mfr, model, type_aircraft, type_engine, category, builder_cert,
                no_eng, no_seats, ac_weight, speed, tc_data_sheet, tc_data_holder
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(code) DO UPDATE SET
                mfr = excluded.mfr,
                model = excluded.model,
                type_aircraft = excluded.type_aircraft,
                type_engine = excluded.type_engine,
                category = excluded.category,
                builder_cert = excluded.builder_cert,
                no_eng = excluded.no_eng,
                no_seats = excluded.no_seats,
                ac_weight = excluded.ac_weight,
                speed = excluded.speed,
                tc_data_sheet = excluded.tc_data_sheet,
                tc_data_holder = excluded.tc_data_holder
             WHERE aircraft_ref.mfr IS DISTINCT FROM excluded.mfr
                OR aircraft_ref.model IS DISTINCT FROM excluded.model
                OR aircraft_ref.type_aircraft IS DISTINCT FROM excluded.type_aircraft
                OR aircraft_ref.type_engine IS DISTINCT FROM excluded.type_engine
                OR aircraft_ref.category IS DISTINCT FROM excluded.category
                OR aircraft_ref.builder_cert IS DISTINCT FROM excluded.builder_cert
                OR aircraft_ref.no_eng IS DISTINCT FROM excluded.no_eng
                OR aircraft_ref.no_seats IS DISTINCT FROM excluded.no_seats
                OR aircraft_ref.ac_weight IS DISTINCT FROM excluded.ac_weight
                OR aircraft_ref.speed IS DISTINCT FROM excluded.speed
                OR aircraft_ref.tc_data_sheet IS DISTINCT FROM excluded.tc_data_sheet
                OR aircraft_ref.tc_data_holder IS DISTINCT FROM excluded.tc_data_holder",
        )?;
        for row in rows {
            keep.execute(params![row.code])?;
            stmt.execute(params![
                row.code,
                row.mfr,
                row.model,
                row.type_aircraft,
                row.type_engine,
                row.category,
                row.builder_cert,
                row.no_eng,
                row.no_seats,
                row.ac_weight,
                row.speed,
                row.tc_data_sheet,
                row.tc_data_holder,
            ])?;
        }
    }
    tx.execute(
        "DELETE FROM aircraft_ref WHERE code NOT IN (SELECT code FROM aircraft_ref_in)",
        [],
    )?;
    tx.execute("DROP TABLE aircraft_ref_in", [])?;
    Ok(())
}

fn upsert_engine_ref(tx: &Transaction<'_>, rows: &[EngineRef]) -> Result<()> {
    tx.execute_batch("CREATE TEMP TABLE engine_ref_in (code TEXT PRIMARY KEY)")?;
    {
        let mut keep = tx.prepare("INSERT OR IGNORE INTO engine_ref_in (code) VALUES (?1)")?;
        let mut stmt = tx.prepare(
            "INSERT INTO engine_ref (code, mfr, model, type_engine, horsepower, thrust)
             VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(code) DO UPDATE SET
                mfr = excluded.mfr,
                model = excluded.model,
                type_engine = excluded.type_engine,
                horsepower = excluded.horsepower,
                thrust = excluded.thrust
             WHERE engine_ref.mfr IS DISTINCT FROM excluded.mfr
                OR engine_ref.model IS DISTINCT FROM excluded.model
                OR engine_ref.type_engine IS DISTINCT FROM excluded.type_engine
                OR engine_ref.horsepower IS DISTINCT FROM excluded.horsepower
                OR engine_ref.thrust IS DISTINCT FROM excluded.thrust",
        )?;
        for row in rows {
            keep.execute(params![row.code])?;
            stmt.execute(params![
                row.code,
                row.mfr,
                row.model,
                row.type_engine,
                row.horsepower,
                row.thrust
            ])?;
        }
    }
    tx.execute(
        "DELETE FROM engine_ref WHERE code NOT IN (SELECT code FROM engine_ref_in)",
        [],
    )?;
    tx.execute("DROP TABLE engine_ref_in", [])?;
    Ok(())
}

fn replace_dealers(tx: &Transaction<'_>, ingest_id: i64, rows: &[DealerRecord]) -> Result<()> {
    tx.execute("DELETE FROM dealers", [])?;
    let mut stmt = tx.prepare(
        "INSERT INTO dealers (
            certificate_number, ownership, certificate_issue_date, expiration_date,
            expiration_flag, cumulative_issue_count, name, street, street2, city, state,
            zip_code, other_names, ingest_id
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
    )?;
    for row in rows {
        stmt.execute(params![
            row.certificate_number,
            row.ownership,
            row.certificate_issue_date,
            row.expiration_date,
            row.expiration_flag,
            row.cumulative_issue_count,
            row.name,
            row.street,
            row.street2,
            row.city,
            row.state,
            row.zip_code,
            row.other_names,
            ingest_id,
        ])?;
    }
    Ok(())
}

fn replace_reserved(tx: &Transaction<'_>, ingest_id: i64, rows: &[ReservedRecord]) -> Result<()> {
    tx.execute("DELETE FROM reserved", [])?;
    let mut stmt = tx.prepare(
        "INSERT INTO reserved (
            n_number, registrant, street, street2, city, state, zip_code, reserve_date,
            type_reservation, expiration_notice_date, n_number_for_change, purge_date, ingest_id
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
    )?;
    for row in rows {
        stmt.execute(params![
            row.n_number,
            row.registrant,
            row.street,
            row.street2,
            row.city,
            row.state,
            row.zip_code,
            row.reserve_date,
            row.type_reservation,
            row.expiration_notice_date,
            row.n_number_for_change,
            row.purge_date,
            ingest_id,
        ])?;
    }
    Ok(())
}

fn load_current(tx: &Transaction<'_>, table: &str) -> Result<HashMap<String, (i64, String)>> {
    let sql = format!("SELECT n_number, id, state_hash FROM {table} WHERE is_current = 1");
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            (row.get::<_, i64>(1)?, row.get::<_, String>(2)?),
        ))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (n, v) = row?;
        map.insert(n, v);
    }
    Ok(map)
}

fn close_row(tx: &Transaction<'_>, table: &str, id: i64, valid_to: &str) -> Result<()> {
    let sql = format!("UPDATE {table} SET is_current = 0, valid_to = ?1 WHERE id = ?2");
    tx.execute(&sql, params![valid_to, id])?;
    Ok(())
}

fn apply_master_scd(
    tx: &Transaction<'_>,
    ingest_id: i64,
    records: &[MasterRecord],
    stats: &mut IngestStats,
) -> Result<()> {
    let mut current = load_current(tx, "aircraft")?;
    let today = utc_date();
    require_utc_date(&today, "valid_from")?;
    let mut seen = HashSet::new();
    let mut insert = tx.prepare(
        "INSERT INTO aircraft (
            n_number, serial_number, mfr_mdl_code, eng_mfr_mdl, year_mfr, type_registrant,
            owner_name, street, street2, city, state, zip_code, region, county, country,
            last_action_date, cert_issue_date, certification, type_aircraft, type_engine,
            status_code, mode_s_code, fractional_owner, air_worth_date, other_names,
            expiration_date, unique_id, kit_mfr, kit_model, mode_s_hex, icao24, state_hash,
            valid_from, valid_to, is_current, ingest_id
         ) VALUES (
            ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,
            ?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,NULL,1,?34
         )",
    )?;

    for rec in records {
        if !seen.insert(rec.n_number.clone()) {
            continue;
        }
        let hash = rec.state_hash();
        match current.remove(&rec.n_number) {
            Some((_, old_hash)) if old_hash == hash => {
                stats.unchanged_rows += 1;
            }
            Some((id, _)) => {
                close_row(tx, "aircraft", id, &today)?;
                insert_master(&mut insert, rec, &hash, &today, ingest_id)?;
                stats.changed_rows += 1;
            }
            None => {
                insert_master(&mut insert, rec, &hash, &today, ingest_id)?;
                stats.new_rows += 1;
            }
        }
    }

    for (_n, (id, _)) in current {
        close_row(tx, "aircraft", id, &today)?;
        stats.closed_rows += 1;
    }
    Ok(())
}

fn insert_master(
    stmt: &mut rusqlite::Statement<'_>,
    rec: &MasterRecord,
    hash: &str,
    valid_from: &str,
    ingest_id: i64,
) -> Result<()> {
    stmt.execute(params![
        rec.n_number,
        rec.serial_number,
        rec.mfr_mdl_code,
        rec.eng_mfr_mdl,
        rec.year_mfr,
        rec.type_registrant,
        rec.owner_name,
        rec.street,
        rec.street2,
        rec.city,
        rec.state,
        rec.zip_code,
        rec.region,
        rec.county,
        rec.country,
        rec.last_action_date,
        rec.cert_issue_date,
        rec.certification,
        rec.type_aircraft,
        rec.type_engine,
        rec.status_code,
        rec.mode_s_code,
        rec.fractional_owner,
        rec.air_worth_date,
        rec.other_names,
        rec.expiration_date,
        rec.unique_id,
        rec.kit_mfr,
        rec.kit_model,
        rec.mode_s_hex,
        canonical_icao24(&rec.mode_s_hex),
        hash,
        valid_from,
        ingest_id,
    ])?;
    Ok(())
}

fn apply_dereg_scd(
    tx: &Transaction<'_>,
    ingest_id: i64,
    records: &[DeregRecord],
    stats: &mut IngestStats,
) -> Result<()> {
    let mut current = load_current(tx, "deregistered")?;
    let today = utc_date();
    require_utc_date(&today, "valid_from")?;
    let mut seen = HashSet::new();
    let mut insert = tx.prepare(
        "INSERT INTO deregistered (
            n_number, serial_number, mfr_mdl_code, status_code, owner_name, street, street2,
            city, state, zip_code, eng_mfr_mdl, year_mfr, certification, region, county,
            country, air_worth_date, cancel_date, mode_s_code, type_registrant, export_country,
            last_action_date, cert_issue_date, physical_street, physical_street2, physical_city,
            physical_state, physical_zip, physical_county, physical_country, other_names,
            kit_mfr, kit_model, mode_s_hex, icao24, state_hash, valid_from, valid_to, is_current, ingest_id
         ) VALUES (
            ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,
            ?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,NULL,1,?38
         )",
    )?;

    for rec in records {
        if !seen.insert(rec.n_number.clone()) {
            continue;
        }
        let hash = rec.state_hash();
        match current.remove(&rec.n_number) {
            Some((_, old_hash)) if old_hash == hash => {}
            Some((id, _)) => {
                close_row(tx, "deregistered", id, &today)?;
                insert_dereg(&mut insert, rec, &hash, &today, ingest_id)?;
                stats.dereg_changed += 1;
            }
            None => {
                insert_dereg(&mut insert, rec, &hash, &today, ingest_id)?;
                stats.dereg_new += 1;
            }
        }
    }

    for (_n, (id, _)) in current {
        close_row(tx, "deregistered", id, &today)?;
        stats.dereg_closed += 1;
    }
    Ok(())
}

fn insert_dereg(
    stmt: &mut rusqlite::Statement<'_>,
    rec: &DeregRecord,
    hash: &str,
    valid_from: &str,
    ingest_id: i64,
) -> Result<()> {
    stmt.execute(params![
        rec.n_number,
        rec.serial_number,
        rec.mfr_mdl_code,
        rec.status_code,
        rec.owner_name,
        rec.street,
        rec.street2,
        rec.city,
        rec.state,
        rec.zip_code,
        rec.eng_mfr_mdl,
        rec.year_mfr,
        rec.certification,
        rec.region,
        rec.county,
        rec.country,
        rec.air_worth_date,
        rec.cancel_date,
        rec.mode_s_code,
        rec.type_registrant,
        rec.export_country,
        rec.last_action_date,
        rec.cert_issue_date,
        rec.physical_street,
        rec.physical_street2,
        rec.physical_city,
        rec.physical_state,
        rec.physical_zip,
        rec.physical_county,
        rec.physical_country,
        rec.other_names,
        rec.kit_mfr,
        rec.kit_model,
        rec.mode_s_hex,
        canonical_icao24(&rec.mode_s_hex),
        hash,
        valid_from,
        ingest_id,
    ])?;
    Ok(())
}

fn insert_documents(
    tx: &Transaction<'_>,
    ingest_id: i64,
    records: &[DocumentRecord],
) -> Result<usize> {
    let now = utc_iso(chrono::Utc::now());
    require_utc_instant(&now, "first_seen")?;
    let mut stmt = tx.prepare(
        "INSERT INTO documents (
            type_collateral, collateral, n_number, party_name, document_id, receipt_date,
            processing_date, correction_date, correction_id, serial_id, doc_type,
            first_seen, ingest_id
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
         ON CONFLICT DO NOTHING",
    )?;
    let mut inserted = 0;
    for rec in records {
        let changed = stmt.execute(params![
            rec.type_collateral,
            rec.collateral,
            rec.n_number,
            rec.party_name,
            rec.document_id,
            rec.receipt_date,
            rec.processing_date,
            rec.correction_date,
            rec.correction_id,
            rec.serial_id,
            rec.doc_type,
            now,
            ingest_id,
        ])?;
        inserted += changed;
    }
    Ok(inserted)
}

fn insert_parse_errors(tx: &Transaction<'_>, ingest_id: i64, errors: &[ParseError]) -> Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO parse_errors (ingest_id, file_name, line_number, raw_line, error)
         VALUES (?1,?2,?3,?4,?5)",
    )?;
    for err in errors {
        stmt.execute(params![
            ingest_id,
            err.file_name,
            err.line_number as i64,
            err.raw_line,
            err.error
        ])?;
    }
    Ok(())
}

fn rebuild_fts(tx: &Transaction<'_>) -> Result<()> {
    crate::db::rebuild_aircraft_fts(tx)
}
