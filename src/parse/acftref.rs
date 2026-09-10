use crate::model::{AircraftRef, ParsedFile};
use crate::parse::fixed_width::{field, parse_lines};

const MIN_LEN: usize = 7;
const HEADER_MARKERS: &[&str] = &["CODE", "MFR"];

pub fn parse_acftref(bytes: &[u8]) -> ParsedFile<AircraftRef> {
    parse_lines(bytes, "ACFTREF.txt", HEADER_MARKERS, parse_row)
}

fn parse_row(line: &[u8]) -> Result<AircraftRef, String> {
    if line.len() < MIN_LEN {
        return Err(format!("row too short ({} bytes)", line.len()));
    }
    let code = field(line, 1, 7);
    if code.is_empty() {
        return Err("missing manufacturer/model code".into());
    }
    Ok(AircraftRef {
        code,
        mfr: field(line, 9, 38),
        model: field(line, 40, 59),
        type_aircraft: field(line, 61, 61),
        type_engine: field(line, 63, 64),
        category: field(line, 66, 66),
        builder_cert: field(line, 68, 68),
        no_eng: field(line, 70, 71),
        no_seats: field(line, 73, 75),
        ac_weight: field(line, 77, 83),
        speed: field(line, 85, 88),
        tc_data_sheet: field(line, 90, 105),
        tc_data_holder: field(line, 107, 157),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::fixed_width::{padded_record, write_field};

    pub(crate) fn fixture_acftref(code: &str, mfr: &str, model: &str) -> Vec<u8> {
        let mut buf = padded_record(158);
        write_field(&mut buf, 1, code);
        write_field(&mut buf, 9, mfr);
        write_field(&mut buf, 40, model);
        write_field(&mut buf, 61, "4");
        write_field(&mut buf, 63, "1");
        write_field(&mut buf, 73, "4");
        buf
    }

    #[test]
    fn parses_make_model() {
        let parsed = parse_acftref(&fixture_acftref("2072703", "CESSNA", "172S"));
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0].mfr, "CESSNA");
        assert_eq!(parsed.records[0].model, "172S");
        assert_eq!(parsed.records[0].type_aircraft, "4");
    }
}
