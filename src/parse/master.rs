use crate::canonical_n_number;
use crate::dates::faa_date;
use crate::model::{join_other_names, MasterRecord, ParsedFile};
use crate::parse::fixed_width::{field, parse_lines};

const MIN_LEN: usize = 57;
const HEADER_MARKERS: &[&str] = &["N-NUMBER", "N-NUM"];

pub fn parse_master(bytes: &[u8]) -> ParsedFile<MasterRecord> {
    parse_lines(bytes, "MASTER.txt", HEADER_MARKERS, parse_row)
}

fn parse_row(line: &[u8]) -> Result<MasterRecord, String> {
    if line.len() < MIN_LEN {
        return Err(format!("row too short ({} bytes)", line.len()));
    }
    let n_number = canonical_n_number(&field(line, 1, 5));
    if n_number.is_empty() {
        return Err("missing N-number".into());
    }
    let other_names = join_other_names(&[
        field(line, 277, 326),
        field(line, 328, 377),
        field(line, 379, 428),
        field(line, 430, 479),
        field(line, 481, 530),
    ]);
    Ok(MasterRecord {
        n_number,
        serial_number: field(line, 7, 36),
        mfr_mdl_code: field(line, 38, 44),
        eng_mfr_mdl: field(line, 46, 50),
        year_mfr: field(line, 52, 55),
        type_registrant: field(line, 57, 57),
        owner_name: field(line, 59, 108),
        street: field(line, 110, 142),
        street2: field(line, 144, 176),
        city: field(line, 178, 195),
        state: field(line, 197, 198),
        zip_code: field(line, 200, 209),
        region: field(line, 211, 211),
        county: field(line, 213, 215),
        country: field(line, 217, 218),
        last_action_date: faa_date(&field(line, 220, 227))?,
        cert_issue_date: faa_date(&field(line, 229, 236))?,
        certification: field(line, 238, 247),
        type_aircraft: field(line, 249, 249),
        type_engine: field(line, 251, 252),
        status_code: field(line, 254, 255),
        mode_s_code: field(line, 257, 264),
        fractional_owner: field(line, 266, 266),
        air_worth_date: faa_date(&field(line, 268, 275))?,
        other_names,
        expiration_date: faa_date(&field(line, 532, 539))?,
        unique_id: field(line, 541, 548),
        kit_mfr: field(line, 550, 579),
        kit_model: field(line, 581, 600),
        mode_s_hex: field(line, 602, 611),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::fixed_width::{padded_record, write_field};

    pub(crate) fn fixture_master(n: &str, owner: &str, serial: &str, state: &str) -> Vec<u8> {
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
        write_field(&mut buf, 249, "4");
        write_field(&mut buf, 251, "1");
        write_field(&mut buf, 254, "V");
        write_field(&mut buf, 602, "A00B1C");
        buf
    }

    #[test]
    fn parses_owner_and_n_number() {
        let line = fixture_master("12345", "BANK OF UTAH TRUSTEE", "17281234", "CO");
        let parsed = parse_master(&line);
        assert!(parsed.errors.is_empty());
        assert_eq!(parsed.records.len(), 1);
        let rec = &parsed.records[0];
        assert_eq!(rec.n_number, "N12345");
        assert_eq!(rec.owner_name, "BANK OF UTAH TRUSTEE");
        assert_eq!(rec.serial_number, "17281234");
        assert_eq!(rec.state, "CO");
        assert_eq!(rec.mfr_mdl_code, "2072703");
        assert_eq!(rec.status_code, "V");
        assert_eq!(rec.mode_s_hex, "A00B1C");
    }

    #[test]
    fn skips_header_and_quarantines_short_rows() {
        let mut data = b"N-NUMBER,SERIAL NUMBER,MFR\n".to_vec();
        data.extend(fixture_master("99ABC", "SMITH JOHN", "SN1", "TX"));
        data.push(b'\n');
        data.extend(b"xx\n");
        let parsed = parse_master(&data);
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0].n_number, "N99ABC");
        assert_eq!(parsed.errors.len(), 1);
        assert!(parsed.errors[0].error.contains("too short"));
    }

    #[test]
    fn skips_utf8_bom_and_header() {
        let mut data = b"\xef\xbb\xbfN-NUMBER,SERIAL NUMBER,MFR\n".to_vec();
        data.extend(fixture_master("100", "BENE MARY D", "5334", "OK"));
        data.push(b'\n');
        let parsed = parse_master(&data);
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0].n_number, "N100");
        assert_eq!(parsed.records[0].owner_name, "BENE MARY D");
    }

    #[test]
    fn garbage_date_quarantines_row() {
        let mut buf = fixture_master("12345", "BANK OF UTAH TRUSTEE", "SN1", "CO");
        write_field(&mut buf, 229, "XXXXXXXX");
        let parsed = parse_master(&buf);
        assert!(parsed.records.is_empty());
        assert_eq!(parsed.errors.len(), 1);
        assert!(parsed.errors[0].error.contains("invalid FAA date"));
    }

    #[test]
    fn owner_comma_does_not_shift_fields() {
        let line = fixture_master("4411", "SMITH, JOHN A", "SN9", "FL");
        let rec = &parse_master(&line).records[0];
        assert_eq!(rec.owner_name, "SMITH, JOHN A");
        assert_eq!(rec.city, "DENVER");
        assert_eq!(rec.state, "FL");
    }
}
