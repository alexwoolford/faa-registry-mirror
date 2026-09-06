use crate::canonical_n_number;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterRecord {
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
    pub region: String,
    pub county: String,
    pub country: String,
    pub last_action_date: String,
    pub cert_issue_date: String,
    pub certification: String,
    pub type_aircraft: String,
    pub type_engine: String,
    pub status_code: String,
    pub mode_s_code: String,
    pub fractional_owner: String,
    pub air_worth_date: String,
    pub other_names: String,
    pub expiration_date: String,
    pub unique_id: String,
    pub kit_mfr: String,
    pub kit_model: String,
    pub mode_s_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AircraftRef {
    pub code: String,
    pub mfr: String,
    pub model: String,
    pub type_aircraft: String,
    pub type_engine: String,
    pub category: String,
    pub builder_cert: String,
    pub no_eng: String,
    pub no_seats: String,
    pub ac_weight: String,
    pub speed: String,
    pub tc_data_sheet: String,
    pub tc_data_holder: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRef {
    pub code: String,
    pub mfr: String,
    pub model: String,
    pub type_engine: String,
    pub horsepower: String,
    pub thrust: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeregRecord {
    pub n_number: String,
    pub serial_number: String,
    pub mfr_mdl_code: String,
    pub status_code: String,
    pub owner_name: String,
    pub street: String,
    pub street2: String,
    pub city: String,
    pub state: String,
    pub zip_code: String,
    pub eng_mfr_mdl: String,
    pub year_mfr: String,
    pub certification: String,
    pub region: String,
    pub county: String,
    pub country: String,
    pub air_worth_date: String,
    pub cancel_date: String,
    pub mode_s_code: String,
    pub type_registrant: String,
    pub export_country: String,
    pub last_action_date: String,
    pub cert_issue_date: String,
    pub physical_street: String,
    pub physical_street2: String,
    pub physical_city: String,
    pub physical_state: String,
    pub physical_zip: String,
    pub physical_county: String,
    pub physical_country: String,
    pub other_names: String,
    pub kit_mfr: String,
    pub kit_model: String,
    pub mode_s_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRecord {
    pub type_collateral: String,
    pub collateral: String,
    pub n_number: String,
    pub party_name: String,
    pub document_id: String,
    pub receipt_date: String,
    pub processing_date: String,
    pub correction_date: String,
    pub correction_id: String,
    pub serial_id: String,
    pub doc_type: String,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub file_name: String,
    pub line_number: usize,
    pub raw_line: Vec<u8>,
    pub error: String,
}

#[derive(Debug)]
pub struct ParsedFile<T> {
    pub records: Vec<T>,
    pub errors: Vec<ParseError>,
}

impl<T> Default for ParsedFile<T> {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl MasterRecord {
    pub fn state_hash(&self) -> String {
        hash_parts(&[
            &self.serial_number,
            &self.mfr_mdl_code,
            &self.eng_mfr_mdl,
            &self.year_mfr,
            &self.type_registrant,
            &self.owner_name,
            &self.street,
            &self.street2,
            &self.city,
            &self.state,
            &self.zip_code,
            &self.region,
            &self.county,
            &self.country,
            &self.last_action_date,
            &self.cert_issue_date,
            &self.certification,
            &self.type_aircraft,
            &self.type_engine,
            &self.status_code,
            &self.mode_s_code,
            &self.fractional_owner,
            &self.air_worth_date,
            &self.other_names,
            &self.expiration_date,
            &self.unique_id,
            &self.kit_mfr,
            &self.kit_model,
            &self.mode_s_hex,
        ])
    }
}

impl DeregRecord {
    pub fn state_hash(&self) -> String {
        hash_parts(&[
            &self.serial_number,
            &self.mfr_mdl_code,
            &self.status_code,
            &self.owner_name,
            &self.street,
            &self.street2,
            &self.city,
            &self.state,
            &self.zip_code,
            &self.eng_mfr_mdl,
            &self.year_mfr,
            &self.certification,
            &self.region,
            &self.county,
            &self.country,
            &self.air_worth_date,
            &self.cancel_date,
            &self.mode_s_code,
            &self.type_registrant,
            &self.export_country,
            &self.last_action_date,
            &self.cert_issue_date,
            &self.physical_street,
            &self.physical_street2,
            &self.physical_city,
            &self.physical_state,
            &self.physical_zip,
            &self.physical_county,
            &self.physical_country,
            &self.other_names,
            &self.kit_mfr,
            &self.kit_model,
            &self.mode_s_hex,
        ])
    }
}

impl DocumentRecord {
    pub fn n_number_from_collateral(type_collateral: &str, collateral: &str) -> String {
        if type_collateral == "1" {
            canonical_n_number(collateral)
        } else {
            String::new()
        }
    }
}

pub fn join_other_names(names: &[String]) -> String {
    names
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("|")
}

fn hash_parts(parts: &[&str]) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().to_string()
}

pub fn decode_type_registrant(code: &str) -> &'static str {
    match code {
        "1" => "Individual",
        "2" => "Partnership",
        "3" => "Corporation",
        "4" => "Co-Owned",
        "5" => "Government",
        "7" => "LLC",
        "8" => "Non-Citizen Corporation",
        "9" => "Non-Citizen Co-Owned",
        _ => "Unknown",
    }
}

pub fn decode_type_aircraft(code: &str) -> &'static str {
    match code {
        "1" => "Glider",
        "2" => "Balloon",
        "3" => "Blimp/Dirigible",
        "4" => "Fixed wing single engine",
        "5" => "Fixed wing multi engine",
        "6" => "Rotorcraft",
        "7" => "Weight-shift-control",
        "8" => "Powered Parachute",
        "9" => "Gyroplane",
        "H" => "Hybrid Lift",
        "O" => "Other",
        _ => "Unknown",
    }
}

pub fn decode_type_engine(code: &str) -> &'static str {
    match code.trim() {
        "0" => "None",
        "1" => "Reciprocating",
        "2" => "Turbo-prop",
        "3" => "Turbo-shaft",
        "4" => "Turbo-jet",
        "5" => "Turbo-fan",
        "6" => "Ramjet",
        "7" => "2 Cycle",
        "8" => "4 Cycle",
        "9" => "Unknown",
        "10" => "Electric",
        "11" => "Rotary",
        _ => "Unknown",
    }
}

pub fn decode_status_code(code: &str) -> &'static str {
    match code.trim() {
        "A" => "Triennial form mailed, not returned",
        "D" => "Expired Dealer",
        "E" => "Registration revoked",
        "M" => "Registered to manufacturer under dealer certificate",
        "N" => "Non-citizen corporation missing flight hour report",
        "R" => "Registration pending",
        "S" => "Second triennial form mailed",
        "T" => "Valid registration (trainee)",
        "V" => "Valid Registration",
        "W" => "Certificate ineffective or invalid",
        "X" => "Enforcement letter",
        "Z" => "Permanent reserved",
        "1" => "Triennial form undeliverable",
        "2" => "N-Number assigned, not yet registered",
        "3" => "N-Number assigned as non-type-certificated, not yet registered",
        "4" => "N-Number assigned as import, not yet registered",
        "5" => "Reserved N-Number",
        "6" => "Administratively canceled",
        "7" => "Sale reported",
        "13" => "Registration expired",
        "16" => "Registration expired – pending cancellation",
        "18" => "Sale reported – canceled",
        "22" => "Revoked – canceled",
        "27" => "Registration expired",
        _ => "See FAA status code",
    }
}
