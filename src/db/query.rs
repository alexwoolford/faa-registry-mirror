use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};

use crate::model::{decode_status_code, decode_type_aircraft, decode_type_engine, decode_type_registrant};
use crate::{canonical_n_number, looks_like_mode_s_hex};

#[derive(Debug, Clone)]
pub struct AircraftVersion {
    pub n_number: String,
    pub serial_number: String,
    pub mfr_mdl_code: String,
    pub eng_mfr_mdl: String,
    pub year_mfr: String,
    pub type_registrant: String,
    pub owner_name: String,
    pub street: String,
    pub street2: String,
    pub city: String,
    pub state: String,
    pub zip_code: String,
    pub country: String,
    pub status_code: String,
    pub mode_s_hex: String,
    pub icao24: String,
    pub other_names: String,
    pub cert_issue_date: String,
    pub expiration_date: String,
    pub type_aircraft: String,
    pub type_engine: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub is_current: bool,
    pub aircraft_mfr: Option<String>,
    pub aircraft_model: Option<String>,
    pub engine_mfr: Option<String>,
    pub engine_model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeregVersion {
    pub n_number: String,
    pub owner_name: String,
    pub cancel_date: String,
    pub export_country: String,
    pub status_code: String,
    pub serial_number: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct DocumentHit {
    pub document_id: String,
    pub type_collateral: String,
    pub collateral: String,
    pub party_name: String,
    pub receipt_date: String,
    pub doc_type: String,
    pub serial_id: String,
}

#[derive(Debug, Clone)]
pub struct LookupResult {
    pub n_number: String,
    pub current: Option<AircraftVersion>,
    pub history: Vec<AircraftVersion>,
    pub deregistered: Vec<DeregVersion>,
    pub documents: Vec<DocumentHit>,
}

#[derive(Debug, Clone)]
pub struct OwnerHit {
    pub n_number: String,
    pub owner_name: String,
    pub city: String,
    pub state: String,
    pub mfr: Option<String>,
    pub model: Option<String>,
    pub status_code: String,
}

#[derive(Debug, Clone)]
pub struct IngestStatus {
    pub id: i64,
    pub as_of_date: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source: String,
    pub zip_hash: Option<String>,
    pub master_rows: Option<i64>,
    pub new_rows: Option<i64>,
    pub changed_rows: Option<i64>,
    pub closed_rows: Option<i64>,
    pub unchanged_rows: Option<i64>,
    pub skipped_rows: Option<i64>,
    pub dereg_new: Option<i64>,
    pub dereg_changed: Option<i64>,
    pub dereg_closed: Option<i64>,
    pub documents_inserted: Option<i64>,
    pub dealer_rows: Option<i64>,
    pub reserved_rows: Option<i64>,
    pub status: String,
    pub error: Option<String>,
}

pub fn lookup(conn: &Connection, n_number: &str) -> Result<LookupResult> {
    let (n, versions) = if looks_like_mode_s_hex(n_number) {
        let hex = n_number.trim().to_ascii_lowercase();
        let versions = load_aircraft_versions_by_hex(conn, &hex)?;
        let n = versions
            .iter()
            .find(|v| v.is_current)
            .or_else(|| versions.first())
            .map(|v| v.n_number.clone())
            .unwrap_or_else(|| hex);
        (n, versions)
    } else {
        let n = canonical_n_number(n_number);
        (n.clone(), load_aircraft_versions(conn, &n)?)
    };
    let mut versions = versions;
    let current_idx = versions.iter().position(|v| v.is_current);
    let current = current_idx.map(|i| versions.remove(i));
    Ok(LookupResult {
        n_number: n.clone(),
        current,
        history: versions,
        deregistered: load_dereg(conn, &n)?,
        documents: load_documents(conn, &n)?,
    })
}

fn load_aircraft_versions(conn: &Connection, n: &str) -> Result<Vec<AircraftVersion>> {
    let mut stmt = conn.prepare(
        "SELECT
            a.n_number, a.serial_number, a.mfr_mdl_code, a.eng_mfr_mdl, a.year_mfr,
            a.type_registrant, a.owner_name, a.street, a.street2, a.city, a.state,
            a.zip_code, a.country, a.status_code, a.mode_s_hex, a.icao24, a.other_names,
            a.cert_issue_date, a.expiration_date, a.type_aircraft, a.type_engine,
            a.valid_from, a.valid_to, a.is_current,
            r.mfr, r.model, e.mfr, e.model
         FROM aircraft a
         LEFT JOIN aircraft_ref r ON r.code = a.mfr_mdl_code
         LEFT JOIN engine_ref e ON e.code = a.eng_mfr_mdl
         WHERE a.n_number = ?1
         ORDER BY a.valid_from DESC",
    )?;
    let rows = stmt.query_map([n], |row| {
        Ok(AircraftVersion {
            n_number: row.get(0)?,
            serial_number: row.get(1)?,
            mfr_mdl_code: row.get(2)?,
            eng_mfr_mdl: row.get(3)?,
            year_mfr: row.get(4)?,
            type_registrant: row.get(5)?,
            owner_name: row.get(6)?,
            street: row.get(7)?,
            street2: row.get(8)?,
            city: row.get(9)?,
            state: row.get(10)?,
            zip_code: row.get(11)?,
            country: row.get(12)?,
            status_code: row.get(13)?,
            mode_s_hex: row.get(14)?,
            icao24: row.get(15)?,
            other_names: row.get(16)?,
            cert_issue_date: row.get(17)?,
            expiration_date: row.get(18)?,
            type_aircraft: row.get(19)?,
            type_engine: row.get(20)?,
            valid_from: row.get(21)?,
            valid_to: row.get(22)?,
            is_current: row.get::<_, i64>(23)? == 1,
            aircraft_mfr: row.get(24)?,
            aircraft_model: row.get(25)?,
            engine_mfr: row.get(26)?,
            engine_model: row.get(27)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().context("load aircraft versions")
}

fn load_aircraft_versions_by_hex(conn: &Connection, hex: &str) -> Result<Vec<AircraftVersion>> {
    let mut stmt = conn.prepare(
        "SELECT
            a.n_number, a.serial_number, a.mfr_mdl_code, a.eng_mfr_mdl, a.year_mfr,
            a.type_registrant, a.owner_name, a.street, a.street2, a.city, a.state,
            a.zip_code, a.country, a.status_code, a.mode_s_hex, a.icao24, a.other_names,
            a.cert_issue_date, a.expiration_date, a.type_aircraft, a.type_engine,
            a.valid_from, a.valid_to, a.is_current,
            r.mfr, r.model, e.mfr, e.model
         FROM aircraft a
         LEFT JOIN aircraft_ref r ON r.code = a.mfr_mdl_code
         LEFT JOIN engine_ref e ON e.code = a.eng_mfr_mdl
         WHERE a.icao24 = ?1 OR lower(a.mode_s_hex) = ?1
         ORDER BY a.valid_from DESC",
    )?;
    let rows = stmt.query_map([hex], |row| {
        Ok(AircraftVersion {
            n_number: row.get(0)?,
            serial_number: row.get(1)?,
            mfr_mdl_code: row.get(2)?,
            eng_mfr_mdl: row.get(3)?,
            year_mfr: row.get(4)?,
            type_registrant: row.get(5)?,
            owner_name: row.get(6)?,
            street: row.get(7)?,
            street2: row.get(8)?,
            city: row.get(9)?,
            state: row.get(10)?,
            zip_code: row.get(11)?,
            country: row.get(12)?,
            status_code: row.get(13)?,
            mode_s_hex: row.get(14)?,
            icao24: row.get(15)?,
            other_names: row.get(16)?,
            cert_issue_date: row.get(17)?,
            expiration_date: row.get(18)?,
            type_aircraft: row.get(19)?,
            type_engine: row.get(20)?,
            valid_from: row.get(21)?,
            valid_to: row.get(22)?,
            is_current: row.get::<_, i64>(23)? == 1,
            aircraft_mfr: row.get(24)?,
            aircraft_model: row.get(25)?,
            engine_mfr: row.get(26)?,
            engine_model: row.get(27)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .context("load aircraft versions by hex")
}

fn load_dereg(conn: &Connection, n: &str) -> Result<Vec<DeregVersion>> {
    let mut stmt = conn.prepare(
        "SELECT n_number, owner_name, cancel_date, export_country, status_code,
                serial_number, valid_from, valid_to, is_current
         FROM deregistered
         WHERE n_number = ?1
         ORDER BY valid_from DESC",
    )?;
    let rows = stmt.query_map([n], |row| {
        Ok(DeregVersion {
            n_number: row.get(0)?,
            owner_name: row.get(1)?,
            cancel_date: row.get(2)?,
            export_country: row.get(3)?,
            status_code: row.get(4)?,
            serial_number: row.get(5)?,
            valid_from: row.get(6)?,
            valid_to: row.get(7)?,
            is_current: row.get::<_, i64>(8)? == 1,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().context("load deregistered")
}

fn load_documents(conn: &Connection, n: &str) -> Result<Vec<DocumentHit>> {
    let mut stmt = conn.prepare(
        "SELECT document_id, type_collateral, collateral, party_name, receipt_date, doc_type, serial_id
         FROM documents
         WHERE n_number = ?1
         ORDER BY receipt_date DESC",
    )?;
    let rows = stmt.query_map([n], |row| {
        Ok(DocumentHit {
            document_id: row.get(0)?,
            type_collateral: row.get(1)?,
            collateral: row.get(2)?,
            party_name: row.get(3)?,
            receipt_date: row.get(4)?,
            doc_type: row.get(5)?,
            serial_id: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().context("load documents")
}

pub fn search_owner(conn: &Connection, name: &str, limit: usize) -> Result<Vec<OwnerHit>> {
    let has_fts: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'aircraft_fts'",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !has_fts {
        anyhow::bail!("no search index yet; run ingest first");
    }
    let query = fts_query(name);
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT a.n_number, a.owner_name, a.city, a.state, r.mfr, r.model, a.status_code
         FROM aircraft_fts f
         JOIN aircraft a ON a.id = f.rowid AND a.is_current = 1
         LEFT JOIN aircraft_ref r ON r.code = a.mfr_mdl_code
         WHERE aircraft_fts MATCH ?1
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![query, limit as i64], |row| {
        Ok(OwnerHit {
            n_number: row.get(0)?,
            owner_name: row.get(1)?,
            city: row.get(2)?,
            state: row.get(3)?,
            mfr: row.get(4)?,
            model: row.get(5)?,
            status_code: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().context("search owner")
}

fn fts_query(name: &str) -> String {
    name.split_whitespace()
        .map(|token| {
            let cleaned: String = token.chars().filter(|c| c.is_alphanumeric()).collect();
            cleaned
        })
        .filter(|t| !t.is_empty())
        .map(|t| format!("{t}*"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub fn latest_status(conn: &Connection) -> Result<Option<IngestStatus>> {
    let mut stmt = conn.prepare(
        "SELECT id, as_of_date, started_at, finished_at, source, zip_hash, master_rows, new_rows,
                changed_rows, closed_rows, unchanged_rows, skipped_rows, dereg_new,
                dereg_changed, dereg_closed, documents_inserted, dealer_rows, reserved_rows,
                status, error
         FROM ingest_runs
         ORDER BY id DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map([], |row| {
        Ok(IngestStatus {
            id: row.get(0)?,
            as_of_date: row.get(1)?,
            started_at: row.get(2)?,
            finished_at: row.get(3)?,
            source: row.get(4)?,
            zip_hash: row.get(5)?,
            master_rows: row.get(6)?,
            new_rows: row.get(7)?,
            changed_rows: row.get(8)?,
            closed_rows: row.get(9)?,
            unchanged_rows: row.get(10)?,
            skipped_rows: row.get(11)?,
            dereg_new: row.get(12)?,
            dereg_changed: row.get(13)?,
            dereg_closed: row.get(14)?,
            documents_inserted: row.get(15)?,
            dealer_rows: row.get(16)?,
            reserved_rows: row.get(17)?,
            status: row.get(18)?,
            error: row.get(19)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn format_lookup(result: &LookupResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", result.n_number));
    match &result.current {
        Some(cur) => {
            out.push_str("\nCurrent registration\n");
            out.push_str(&format_aircraft(cur));
        }
        None => out.push_str("\nNo current MASTER registration.\n"),
    }
    if !result.history.is_empty() {
        out.push_str("\nOwnership history\n");
        for ver in &result.history {
            let until = ver.valid_to.as_deref().unwrap_or("open");
            out.push_str(&format!(
                "  {} .. {}  {}  ({})\n",
                ver.valid_from, until, ver.owner_name, ver.city
            ));
        }
    }
    if !result.deregistered.is_empty() {
        out.push_str("\nDeregistration\n");
        for d in &result.deregistered {
            let flag = if d.is_current { "current" } else { "historical" };
            out.push_str(&format!(
                "  [{flag}] cancel {}  owner {}  export {}  status {}\n",
                empty(&d.cancel_date),
                empty(&d.owner_name),
                empty(&d.export_country),
                d.status_code
            ));
        }
    }
    if !result.documents.is_empty() {
        out.push_str("\nDocuments\n");
        for doc in &result.documents {
            out.push_str(&format!(
                "  {}  {}  party={}  type={}\n",
                empty(&doc.receipt_date),
                empty(&doc.document_id),
                empty(&doc.party_name),
                empty(&doc.doc_type)
            ));
        }
    }
    out
}

fn format_aircraft(cur: &AircraftVersion) -> String {
    let make = match (&cur.aircraft_mfr, &cur.aircraft_model) {
        (Some(mfr), Some(model)) if !mfr.is_empty() => format!("{mfr} {model}"),
        _ => format!("code {}", empty(&cur.mfr_mdl_code)),
    };
    let engine = match (&cur.engine_mfr, &cur.engine_model) {
        (Some(mfr), Some(model)) if !mfr.is_empty() => format!("{mfr} {model}"),
        _ => format!("code {}", empty(&cur.eng_mfr_mdl)),
    };
    let street = [cur.street.as_str(), cur.street2.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "  Owner:      {} ({})\n  Address:    {}, {}, {} {}\n  Country:    {}\n  Aircraft:   {} ({})\n  Engine:     {} ({})\n  Year:       {}\n  Serial:     {}\n  Status:     {} ({})\n  Mode S hex: {}\n  icao24:     {}\n  Co-owners:  {}\n  Cert issue: {}\n  Expires:    {}\n  Valid from: {}\n",
        empty(&cur.owner_name),
        decode_type_registrant(&cur.type_registrant),
        empty(&street),
        empty(&cur.city),
        empty(&cur.state),
        empty(&cur.zip_code),
        empty(&cur.country),
        make,
        decode_type_aircraft(&cur.type_aircraft),
        engine,
        decode_type_engine(&cur.type_engine),
        empty(&cur.year_mfr),
        empty(&cur.serial_number),
        empty(&cur.status_code),
        decode_status_code(&cur.status_code),
        empty(&cur.mode_s_hex),
        empty(&cur.icao24),
        empty(&cur.other_names),
        empty(&cur.cert_issue_date),
        empty(&cur.expiration_date),
        cur.valid_from,
    )
}

pub fn format_owner_hits(hits: &[OwnerHit]) -> String {
    if hits.is_empty() {
        return "No matching current owners.\n".into();
    }
    let mut out = String::new();
    for hit in hits {
        let plane = match (&hit.mfr, &hit.model) {
            (Some(mfr), Some(model)) if !mfr.is_empty() => format!("{mfr} {model}"),
            _ => String::new(),
        };
        out.push_str(&format!(
            "{:<8}  {:<40}  {}, {}  {}  status {}\n",
            hit.n_number,
            hit.owner_name,
            hit.city,
            hit.state,
            plane,
            hit.status_code
        ));
    }
    out
}

pub fn format_status(status: &IngestStatus) -> String {
    format!(
        "Ingest #{}\n  status:     {}\n  as_of:      {}\n  started:    {}\n  finished:   {}\n  source:     {}\n  zip hash:   {}\n  master:     {}\n  new/changed/closed/unchanged: {} / {} / {} / {}\n  skipped:    {}\n  dereg new/changed/closed: {} / {} / {}\n  documents:  {}\n  dealers:    {}\n  reserved:   {}\n  error:      {}\n",
        status.id,
        status.status,
        status.as_of_date.as_deref().unwrap_or("-"),
        status.started_at,
        status.finished_at.as_deref().unwrap_or("-"),
        status.source,
        status.zip_hash.as_deref().unwrap_or("-"),
        fmt_opt(status.master_rows),
        fmt_opt(status.new_rows),
        fmt_opt(status.changed_rows),
        fmt_opt(status.closed_rows),
        fmt_opt(status.unchanged_rows),
        fmt_opt(status.skipped_rows),
        fmt_opt(status.dereg_new),
        fmt_opt(status.dereg_changed),
        fmt_opt(status.dereg_closed),
        fmt_opt(status.documents_inserted),
        fmt_opt(status.dealer_rows),
        fmt_opt(status.reserved_rows),
        status.error.as_deref().unwrap_or("-"),
    )
}

fn empty(s: &str) -> &str {
    if s.is_empty() {
        "-"
    } else {
        s
    }
}

fn fmt_opt(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "-".into())
}
