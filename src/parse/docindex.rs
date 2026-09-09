use crate::dates::faa_date;
use crate::model::{DocumentRecord, ParseError, ParsedFile};
use crate::parse::fixed_width::{field, looks_like_header, strip_bom, strip_line_ending};

const MIN_LEN: usize = 90;
const HEADER_MARKERS: &[&str] = &["TYPE", "COLLATERAL"];

pub fn parse_docindex(bytes: &[u8]) -> ParsedFile<DocumentRecord> {
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
                file_name: "DOCINDEX.txt".into(),
                line_number: idx + 1,
                raw_line: line.to_vec(),
                error,
            }),
        }
    }
    out
}

fn parse_row(line: &[u8]) -> Result<DocumentRecord, String> {
    if line.len() < MIN_LEN {
        return Err(format!("row too short ({} bytes)", line.len()));
    }
    let type_collateral = field(line, 1, 1);
    let collateral = field(line, 3, 39);
    if type_collateral.is_empty() {
        return Err("missing collateral type".into());
    }
    // Official length was 164; Doc Type was appended on 2024-07-30.
    let doc_type = if line.len() > 164 {
        field(line, 165, line.len())
    } else {
        String::new()
    };
    Ok(DocumentRecord {
        n_number: DocumentRecord::n_number_from_collateral(&type_collateral, &collateral),
        type_collateral,
        collateral,
        party_name: field(line, 41, 90),
        document_id: field(line, 92, 103),
        receipt_date: faa_date(&field(line, 105, 112))?,
        processing_date: faa_date(&field(line, 114, 121))?,
        correction_date: faa_date(&field(line, 123, 130))?,
        correction_id: field(line, 132, 132),
        serial_id: field(line, 134, 163),
        doc_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::fixed_width::{padded_record, write_field};

    pub(crate) fn fixture_docindex(n: &str, party: &str, doc_id: &str, doc_type: &str) -> Vec<u8> {
        let mut buf = padded_record(180);
        write_field(&mut buf, 1, "1");
        write_field(&mut buf, 3, n);
        write_field(&mut buf, 41, party);
        write_field(&mut buf, 92, doc_id);
        write_field(&mut buf, 105, "20240601");
        write_field(&mut buf, 165, doc_type);
        buf
    }

    #[test]
    fn extracts_n_number_and_trailing_doc_type() {
        let parsed = parse_docindex(&fixture_docindex(
            "12345",
            "BANK, N.A.",
            "DOC000111222",
            "SECURITY",
        ));
        assert_eq!(parsed.records.len(), 1);
        let rec = &parsed.records[0];
        assert_eq!(rec.n_number, "N12345");
        assert_eq!(rec.party_name, "BANK, N.A.");
        assert_eq!(rec.document_id, "DOC000111222");
        assert_eq!(rec.doc_type, "SECURITY");
        assert_eq!(rec.receipt_date, "2024-06-01");
    }

    #[test]
    fn engine_lien_has_empty_n_number() {
        let mut buf = padded_record(164);
        write_field(&mut buf, 1, "2");
        write_field(&mut buf, 3, "LYCOMING IO-360 SN99");
        write_field(&mut buf, 41, "LIENHOLDER INC");
        write_field(&mut buf, 92, "DOC999");
        let rec = &parse_docindex(&buf).records[0];
        assert!(rec.n_number.is_empty());
        assert_eq!(rec.collateral, "LYCOMING IO-360 SN99");
    }
}
