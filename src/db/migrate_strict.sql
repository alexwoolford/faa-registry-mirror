-- Recreate real tables as STRICT. FTS must drop first (content='aircraft').
-- ingest_runs uses a named INSERT because ALTER added dealer_rows/reserved_rows
-- after status/error on older files.
DROP TABLE IF EXISTS aircraft_fts;

CREATE TABLE ingest_runs_new (
    id INTEGER PRIMARY KEY,
    as_of_date TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    source TEXT NOT NULL,
    zip_hash TEXT,
    master_rows INTEGER,
    new_rows INTEGER,
    changed_rows INTEGER,
    closed_rows INTEGER,
    unchanged_rows INTEGER,
    skipped_rows INTEGER,
    dereg_new INTEGER,
    dereg_changed INTEGER,
    dereg_closed INTEGER,
    documents_inserted INTEGER,
    dealer_rows INTEGER,
    reserved_rows INTEGER,
    status TEXT NOT NULL CHECK (status IN ('running', 'ok', 'failed', 'skipped')),
    error TEXT
) STRICT;
INSERT INTO ingest_runs_new (
    id, as_of_date, started_at, finished_at, source, zip_hash,
    master_rows, new_rows, changed_rows, closed_rows, unchanged_rows, skipped_rows,
    dereg_new, dereg_changed, dereg_closed, documents_inserted,
    dealer_rows, reserved_rows, status, error
)
SELECT
    id, as_of_date, started_at, finished_at, source, zip_hash,
    master_rows, new_rows, changed_rows, closed_rows, unchanged_rows, skipped_rows,
    dereg_new, dereg_changed, dereg_closed, documents_inserted,
    dealer_rows, reserved_rows, status, error
FROM ingest_runs;

CREATE TABLE parse_errors_new (
    id INTEGER PRIMARY KEY,
    ingest_id INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    raw_line BLOB NOT NULL,
    error TEXT NOT NULL,
    FOREIGN KEY (ingest_id) REFERENCES ingest_runs(id)
) STRICT;
INSERT INTO parse_errors_new SELECT * FROM parse_errors;

CREATE TABLE aircraft_ref_new (
    code TEXT PRIMARY KEY,
    mfr TEXT NOT NULL,
    model TEXT NOT NULL,
    type_aircraft TEXT NOT NULL,
    type_engine TEXT NOT NULL,
    category TEXT NOT NULL,
    builder_cert TEXT NOT NULL,
    no_eng TEXT NOT NULL,
    no_seats TEXT NOT NULL,
    ac_weight TEXT NOT NULL,
    speed TEXT NOT NULL,
    tc_data_sheet TEXT NOT NULL,
    tc_data_holder TEXT NOT NULL
) STRICT;
INSERT INTO aircraft_ref_new SELECT * FROM aircraft_ref;

CREATE TABLE engine_ref_new (
    code TEXT PRIMARY KEY,
    mfr TEXT NOT NULL,
    model TEXT NOT NULL,
    type_engine TEXT NOT NULL,
    horsepower TEXT NOT NULL,
    thrust TEXT NOT NULL
) STRICT;
INSERT INTO engine_ref_new SELECT * FROM engine_ref;

CREATE TABLE aircraft_new (
    id INTEGER PRIMARY KEY,
    n_number TEXT NOT NULL,
    serial_number TEXT NOT NULL,
    mfr_mdl_code TEXT NOT NULL,
    eng_mfr_mdl TEXT NOT NULL,
    year_mfr TEXT NOT NULL,
    type_registrant TEXT NOT NULL,
    owner_name TEXT NOT NULL,
    street TEXT NOT NULL,
    street2 TEXT NOT NULL,
    city TEXT NOT NULL,
    state TEXT NOT NULL,
    zip_code TEXT NOT NULL,
    region TEXT NOT NULL,
    county TEXT NOT NULL,
    country TEXT NOT NULL,
    last_action_date TEXT NOT NULL,
    cert_issue_date TEXT NOT NULL,
    certification TEXT NOT NULL,
    type_aircraft TEXT NOT NULL,
    type_engine TEXT NOT NULL,
    status_code TEXT NOT NULL,
    mode_s_code TEXT NOT NULL,
    fractional_owner TEXT NOT NULL,
    air_worth_date TEXT NOT NULL,
    other_names TEXT NOT NULL,
    expiration_date TEXT NOT NULL,
    unique_id TEXT NOT NULL,
    kit_mfr TEXT NOT NULL,
    kit_model TEXT NOT NULL,
    mode_s_hex TEXT NOT NULL,
    icao24 TEXT NOT NULL,
    state_hash TEXT NOT NULL,
    valid_from TEXT NOT NULL,
    valid_to TEXT,
    is_current INTEGER NOT NULL DEFAULT 1,
    ingest_id INTEGER NOT NULL,
    FOREIGN KEY (ingest_id) REFERENCES ingest_runs(id)
) STRICT;
INSERT INTO aircraft_new SELECT * FROM aircraft;

CREATE TABLE deregistered_new (
    id INTEGER PRIMARY KEY,
    n_number TEXT NOT NULL,
    serial_number TEXT NOT NULL,
    mfr_mdl_code TEXT NOT NULL,
    status_code TEXT NOT NULL,
    owner_name TEXT NOT NULL,
    street TEXT NOT NULL,
    street2 TEXT NOT NULL,
    city TEXT NOT NULL,
    state TEXT NOT NULL,
    zip_code TEXT NOT NULL,
    eng_mfr_mdl TEXT NOT NULL,
    year_mfr TEXT NOT NULL,
    certification TEXT NOT NULL,
    region TEXT NOT NULL,
    county TEXT NOT NULL,
    country TEXT NOT NULL,
    air_worth_date TEXT NOT NULL,
    cancel_date TEXT NOT NULL,
    mode_s_code TEXT NOT NULL,
    type_registrant TEXT NOT NULL,
    export_country TEXT NOT NULL,
    last_action_date TEXT NOT NULL,
    cert_issue_date TEXT NOT NULL,
    physical_street TEXT NOT NULL,
    physical_street2 TEXT NOT NULL,
    physical_city TEXT NOT NULL,
    physical_state TEXT NOT NULL,
    physical_zip TEXT NOT NULL,
    physical_county TEXT NOT NULL,
    physical_country TEXT NOT NULL,
    other_names TEXT NOT NULL,
    kit_mfr TEXT NOT NULL,
    kit_model TEXT NOT NULL,
    mode_s_hex TEXT NOT NULL,
    icao24 TEXT NOT NULL,
    state_hash TEXT NOT NULL,
    valid_from TEXT NOT NULL,
    valid_to TEXT,
    is_current INTEGER NOT NULL DEFAULT 1,
    ingest_id INTEGER NOT NULL,
    FOREIGN KEY (ingest_id) REFERENCES ingest_runs(id)
) STRICT;
INSERT INTO deregistered_new SELECT * FROM deregistered;

CREATE TABLE documents_new (
    id INTEGER PRIMARY KEY,
    type_collateral TEXT NOT NULL,
    collateral TEXT NOT NULL,
    n_number TEXT NOT NULL,
    party_name TEXT NOT NULL,
    document_id TEXT NOT NULL,
    receipt_date TEXT NOT NULL,
    processing_date TEXT NOT NULL,
    correction_date TEXT NOT NULL,
    correction_id TEXT NOT NULL,
    serial_id TEXT NOT NULL,
    doc_type TEXT NOT NULL,
    first_seen TEXT NOT NULL,
    ingest_id INTEGER NOT NULL,
    FOREIGN KEY (ingest_id) REFERENCES ingest_runs(id),
    UNIQUE (
        document_id,
        type_collateral,
        collateral,
        party_name,
        receipt_date,
        serial_id,
        doc_type
    )
) STRICT;
INSERT INTO documents_new SELECT * FROM documents;

CREATE TABLE reserved_new (
    n_number TEXT PRIMARY KEY,
    registrant TEXT NOT NULL,
    street TEXT NOT NULL,
    street2 TEXT NOT NULL,
    city TEXT NOT NULL,
    state TEXT NOT NULL,
    zip_code TEXT NOT NULL,
    reserve_date TEXT NOT NULL,
    type_reservation TEXT NOT NULL,
    expiration_notice_date TEXT NOT NULL,
    n_number_for_change TEXT NOT NULL,
    purge_date TEXT NOT NULL,
    ingest_id INTEGER NOT NULL,
    FOREIGN KEY (ingest_id) REFERENCES ingest_runs(id)
) STRICT;
INSERT INTO reserved_new SELECT * FROM reserved;

CREATE TABLE dealers_new (
    certificate_number TEXT PRIMARY KEY,
    ownership TEXT NOT NULL,
    certificate_issue_date TEXT NOT NULL,
    expiration_date TEXT NOT NULL,
    expiration_flag TEXT NOT NULL,
    cumulative_issue_count TEXT NOT NULL,
    name TEXT NOT NULL,
    street TEXT NOT NULL,
    street2 TEXT NOT NULL,
    city TEXT NOT NULL,
    state TEXT NOT NULL,
    zip_code TEXT NOT NULL,
    other_names TEXT NOT NULL,
    ingest_id INTEGER NOT NULL,
    FOREIGN KEY (ingest_id) REFERENCES ingest_runs(id)
) STRICT;
INSERT INTO dealers_new SELECT * FROM dealers;

DROP TABLE parse_errors;
DROP TABLE aircraft;
DROP TABLE deregistered;
DROP TABLE documents;
DROP TABLE reserved;
DROP TABLE dealers;
DROP TABLE ingest_runs;
DROP TABLE aircraft_ref;
DROP TABLE engine_ref;

ALTER TABLE ingest_runs_new RENAME TO ingest_runs;
ALTER TABLE parse_errors_new RENAME TO parse_errors;
ALTER TABLE aircraft_ref_new RENAME TO aircraft_ref;
ALTER TABLE engine_ref_new RENAME TO engine_ref;
ALTER TABLE aircraft_new RENAME TO aircraft;
ALTER TABLE deregistered_new RENAME TO deregistered;
ALTER TABLE documents_new RENAME TO documents;
ALTER TABLE reserved_new RENAME TO reserved;
ALTER TABLE dealers_new RENAME TO dealers;
