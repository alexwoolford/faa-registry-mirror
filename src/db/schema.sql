CREATE TABLE IF NOT EXISTS ingest_runs (
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
    status TEXT NOT NULL,
    error TEXT
);

CREATE TABLE IF NOT EXISTS parse_errors (
    id INTEGER PRIMARY KEY,
    ingest_id INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    raw_line BLOB NOT NULL,
    error TEXT NOT NULL,
    FOREIGN KEY (ingest_id) REFERENCES ingest_runs(id)
);

CREATE TABLE IF NOT EXISTS aircraft_ref (
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
);

CREATE TABLE IF NOT EXISTS engine_ref (
    code TEXT PRIMARY KEY,
    mfr TEXT NOT NULL,
    model TEXT NOT NULL,
    type_engine TEXT NOT NULL,
    horsepower TEXT NOT NULL,
    thrust TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS aircraft (
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
);

CREATE INDEX IF NOT EXISTS idx_aircraft_n_number ON aircraft(n_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_aircraft_current
    ON aircraft(n_number) WHERE is_current = 1;
CREATE INDEX IF NOT EXISTS idx_aircraft_owner ON aircraft(owner_name);
CREATE INDEX IF NOT EXISTS idx_aircraft_icao24 ON aircraft(icao24);

CREATE TABLE IF NOT EXISTS deregistered (
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
);

CREATE INDEX IF NOT EXISTS idx_dereg_n_number ON deregistered(n_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dereg_current
    ON deregistered(n_number) WHERE is_current = 1;
CREATE INDEX IF NOT EXISTS idx_dereg_icao24 ON deregistered(icao24);

CREATE TABLE IF NOT EXISTS documents (
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
);

CREATE INDEX IF NOT EXISTS idx_documents_n_number ON documents(n_number);

CREATE TABLE IF NOT EXISTS reserved (
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
);

CREATE INDEX IF NOT EXISTS idx_reserved_registrant ON reserved(registrant);

CREATE TABLE IF NOT EXISTS dealers (
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
);

CREATE INDEX IF NOT EXISTS idx_dealers_name ON dealers(name);

-- External-content FTS: no aircraft_fts_content / c0–c3 shadow table.
-- Query this virtual table (or JOIN aircraft ON aircraft.id = aircraft_fts.rowid).
-- Rebuild is a full replace in ingest; CREATE here so an empty DB still has the object.
CREATE VIRTUAL TABLE IF NOT EXISTS aircraft_fts USING fts5(
    n_number UNINDEXED,
    owner_name,
    city,
    state,
    content='aircraft',
    content_rowid='id'
);
