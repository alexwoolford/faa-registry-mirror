use crate::canonical_n_number;
use crate::dates::faa_date;
use crate::model::{ParseError, ParsedFile, ReservedRecord};
use crate::parse::fixed_width::{field, looks_like_header, strip_bom, strip_line_ending};

const MIN_LEN: usize = 5;
const HEADER_MARKERS: &[&str] = &["N-NUMBER", "N-NUM"];

pub fn parse_reserved(bytes: &[u8]) -> ParsedFile<ReservedRecord> {
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
                file_name: "RESERVED.txt".into(),
                line_number: idx + 1,
                raw_line: line.to_vec(),
                error,
            }),
        }
    }
    out
}

fn parse_row(line: &[u8]) -> Result<ReservedRecord, String> {
    if line.len() < MIN_LEN {
        return Err(format!("row too short ({} bytes)", line.len()));
    }
    let n_number = canonical_n_number(&field(line, 1, 5));
    if n_number.is_empty() {
        return Err("missing N-number".into());
    }
    let change = field(line, 180, 184);
    Ok(ReservedRecord {
        n_number,
        registrant: field(line, 7, 56),
        street: field(line, 58, 90),
        street2: field(line, 92, 124),
        city: field(line, 126, 143),
        state: field(line, 145, 146),
        zip_code: field(line, 148, 157),
        reserve_date: faa_date(&field(line, 159, 166))?,
        type_reservation: field(line, 168, 169),
        expiration_notice_date: faa_date(&field(line, 171, 178))?,
        n_number_for_change: if change.is_empty() {
            String::new()
        } else {
            canonical_n_number(&change)
        },
        // ardata.pdf says 185-192 / RECORD 192. Live dump is 194 bytes:
        // comma after N-Number for Change, purge YYYYMMDD at 186-193, trailing comma.
        purge_date: faa_date(&field(line, 186, 193))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::fixed_width::{padded_record, write_field};

    pub(crate) fn fixture_reserved(n: &str, registrant: &str, kind: &str) -> Vec<u8> {
        let mut buf = padded_record(194);
        write_field(&mut buf, 1, n);
        write_field(&mut buf, 7, registrant);
        write_field(&mut buf, 58, "1 META WAY");
        write_field(&mut buf, 126, "MENLO PARK");
        write_field(&mut buf, 145, "CA");
        write_field(&mut buf, 148, "94025");
        write_field(&mut buf, 159, "20240115");
        write_field(&mut buf, 168, kind);
        write_field(&mut buf, 171, "20260115");
        write_field(&mut buf, 180, "99ABC");
        write_field(&mut buf, 186, "20270305");
        buf
    }

    #[test]
    fn parses_reservation_and_canonical_n() {
        let parsed = parse_reserved(&fixture_reserved("1WM", "META PLATFORMS INC", "FP"));
        assert!(parsed.errors.is_empty());
        let rec = &parsed.records[0];
        assert_eq!(rec.n_number, "N1WM");
        assert_eq!(rec.registrant, "META PLATFORMS INC");
        assert_eq!(rec.city, "MENLO PARK");
        assert_eq!(rec.state, "CA");
        assert_eq!(rec.reserve_date, "2024-01-15");
        assert_eq!(rec.type_reservation, "FP");
        assert_eq!(rec.expiration_notice_date, "2026-01-15");
        assert_eq!(rec.n_number_for_change, "N99ABC");
        assert_eq!(rec.purge_date, "2027-03-05");
    }

    #[test]
    fn skips_header_and_quarantines_short_rows() {
        let mut data = b"N-NUMBER,REGISTRANT\n".to_vec();
        data.extend(fixture_reserved("66CL", "HOLDING LLC", "AA"));
        data.push(b'\n');
        data.extend(b"xx\n");
        let parsed = parse_reserved(&data);
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0].n_number, "N66CL");
        assert_eq!(parsed.errors.len(), 1);
    }
}
