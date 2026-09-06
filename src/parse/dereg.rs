use crate::canonical_n_number;
use crate::dates::faa_date;
use crate::model::{join_other_names, DeregRecord, ParseError, ParsedFile};
use crate::parse::fixed_width::{field, looks_like_header, strip_bom, strip_line_ending};

const MIN_LEN: usize = 48;
const HEADER_MARKERS: &[&str] = &["N-NUMBER", "N-NUM"];

pub fn parse_dereg(bytes: &[u8]) -> ParsedFile<DeregRecord> {
    let bytes = strip_bom(bytes);
    let mut out = ParsedFile::default();
    for (idx, raw) in bytes.split(|b| *b == b'\n').enumerate() {
        let line = strip_line_ending(raw);
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        if idx == 0 && looks_like_header(line, HEADER_MARKERS) {
            continue;
        }
        match parse_row(line) {
            Ok(record) => out.records.push(record),
            Err(error) => out.errors.push(ParseError {
                file_name: "DEREG.txt".into(),
                line_number: idx + 1,
                raw_line: line.to_vec(),
                error,
            }),
        }
    }
    out
}

fn parse_row(line: &[u8]) -> Result<DeregRecord, String> {
    if line.len() < MIN_LEN {
        return Err(format!("row too short ({} bytes)", line.len()));
    }
    let n_number = canonical_n_number(&field(line, 1, 5));
    if n_number.is_empty() {
        return Err("missing N-number".into());
    }
    let other_names = join_other_names(&[
        field(line, 406, 455),
        field(line, 457, 506),
        field(line, 508, 557),
        field(line, 559, 608),
        field(line, 610, 659),
    ]);
    Ok(DeregRecord {
        n_number,
        serial_number: field(line, 7, 36),
        mfr_mdl_code: field(line, 38, 44),
        status_code: field(line, 46, 47),
        owner_name: field(line, 49, 98),
        street: field(line, 100, 132),
        street2: field(line, 134, 166),
        city: field(line, 168, 185),
        state: field(line, 187, 188),
        zip_code: field(line, 190, 199),
        eng_mfr_mdl: field(line, 201, 205),
        year_mfr: field(line, 207, 210),
        certification: field(line, 212, 221),
        region: field(line, 223, 223),
        county: field(line, 225, 227),
        country: field(line, 229, 230),
        air_worth_date: faa_date(&field(line, 232, 239))?,
        cancel_date: faa_date(&field(line, 241, 248))?,
        mode_s_code: field(line, 250, 257),
        type_registrant: field(line, 259, 259),
        export_country: field(line, 261, 278),
        last_action_date: faa_date(&field(line, 280, 287))?,
        cert_issue_date: faa_date(&field(line, 289, 296))?,
        physical_street: field(line, 298, 330),
        physical_street2: field(line, 332, 364),
        physical_city: field(line, 366, 383),
        physical_state: field(line, 385, 386),
        physical_zip: field(line, 388, 397),
        physical_county: field(line, 399, 401),
        physical_country: field(line, 403, 404),
        other_names,
        kit_mfr: field(line, 661, 690),
        kit_model: field(line, 692, 711),
        mode_s_hex: field(line, 712, 721),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::fixed_width::{padded_record, write_field};

    pub(crate) fn fixture_dereg(n: &str, owner: &str, cancel: &str) -> Vec<u8> {
        let mut buf = padded_record(723);
        write_field(&mut buf, 1, n);
        write_field(&mut buf, 7, "SN-DREG");
        write_field(&mut buf, 38, "2072703");
        write_field(&mut buf, 46, "6");
        write_field(&mut buf, 49, owner);
        write_field(&mut buf, 241, cancel);
        write_field(&mut buf, 261, "CANADA");
        write_field(&mut buf, 712, "A0DEAD");
        buf
    }

    #[test]
    fn parses_cancel_and_export() {
        let parsed = parse_dereg(&fixture_dereg("55555", "OLD OWNER LLC", "20240115"));
        assert_eq!(parsed.records.len(), 1);
        let rec = &parsed.records[0];
        assert_eq!(rec.n_number, "N55555");
        assert_eq!(rec.owner_name, "OLD OWNER LLC");
        assert_eq!(rec.cancel_date, "2024-01-15");
        assert_eq!(rec.export_country, "CANADA");
        assert_eq!(rec.status_code, "6");
    }
}
