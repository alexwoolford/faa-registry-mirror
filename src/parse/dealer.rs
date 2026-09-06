use crate::dates::faa_date;
use crate::model::{join_other_names, DealerRecord, ParseError, ParsedFile};
use crate::parse::fixed_width::{field, looks_like_header, strip_bom, strip_line_ending};

const MIN_LEN: usize = 7;
const HEADER_MARKERS: &[&str] = &["CERTIFICATE", "CERT"];
const OTHER_NAME_START: usize = 191;
const OTHER_NAME_WIDTH: usize = 50;
const OTHER_NAME_STRIDE: usize = 51;
const OTHER_NAME_COUNT: usize = 25;

pub fn parse_dealer(bytes: &[u8]) -> ParsedFile<DealerRecord> {
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
                file_name: "DEALER.txt".into(),
                line_number: idx + 1,
                raw_line: line.to_vec(),
                error,
            }),
        }
    }
    out
}

fn parse_row(line: &[u8]) -> Result<DealerRecord, String> {
    if line.len() < MIN_LEN {
        return Err(format!("row too short ({} bytes)", line.len()));
    }
    let certificate_number = field(line, 1, 7);
    if certificate_number.is_empty() {
        return Err("missing certificate number".into());
    }
    let mut names = Vec::with_capacity(OTHER_NAME_COUNT);
    for i in 0..OTHER_NAME_COUNT {
        let start = OTHER_NAME_START + i * OTHER_NAME_STRIDE;
        names.push(field(line, start, start + OTHER_NAME_WIDTH - 1));
    }
    Ok(DealerRecord {
        certificate_number,
        ownership: field(line, 9, 9),
        certificate_issue_date: faa_date(&field(line, 11, 18))?,
        expiration_date: faa_date(&field(line, 20, 27))?,
        expiration_flag: field(line, 29, 29),
        cumulative_issue_count: field(line, 31, 34),
        name: field(line, 36, 85),
        street: field(line, 87, 119),
        street2: field(line, 121, 153),
        city: field(line, 155, 172),
        state: field(line, 174, 175),
        zip_code: field(line, 177, 186),
        other_names: join_other_names(&names),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::fixed_width::{padded_record, write_field};

    pub(crate) fn fixture_dealer(cert: &str, name: &str, other: &str) -> Vec<u8> {
        let mut buf = padded_record(1465);
        write_field(&mut buf, 1, cert);
        write_field(&mut buf, 9, "7");
        write_field(&mut buf, 11, "20230101");
        write_field(&mut buf, 20, "20251231");
        write_field(&mut buf, 31, "0003");
        write_field(&mut buf, 36, name);
        write_field(&mut buf, 87, "100 DEALER RD");
        write_field(&mut buf, 155, "WICHITA");
        write_field(&mut buf, 174, "KS");
        write_field(&mut buf, 177, "67210");
        write_field(&mut buf, 191, other);
        buf
    }

    #[test]
    fn parses_certificate_and_other_names() {
        let parsed = parse_dealer(&fixture_dealer("26-0001", "CESSNA AIRCRAFT CO", "TEXTRON AVIATION"));
        assert!(parsed.errors.is_empty());
        let rec = &parsed.records[0];
        assert_eq!(rec.certificate_number, "26-0001");
        assert_eq!(rec.ownership, "7");
        assert_eq!(rec.certificate_issue_date, "2023-01-01");
        assert_eq!(rec.expiration_date, "2025-12-31");
        assert_eq!(rec.name, "CESSNA AIRCRAFT CO");
        assert_eq!(rec.city, "WICHITA");
        assert_eq!(rec.other_names, "TEXTRON AVIATION");
    }

    #[test]
    fn name_comma_does_not_shift_fields() {
        let parsed = parse_dealer(&fixture_dealer("26-0002", "SMITH, JANE LLC", ""));
        let rec = &parsed.records[0];
        assert_eq!(rec.name, "SMITH, JANE LLC");
        assert_eq!(rec.state, "KS");
    }
}
