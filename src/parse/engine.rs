use crate::model::{EngineRef, ParsedFile};
use crate::parse::fixed_width::{field, parse_lines};

const MIN_LEN: usize = 5;
const HEADER_MARKERS: &[&str] = &["CODE"];

pub fn parse_engine(bytes: &[u8]) -> ParsedFile<EngineRef> {
    parse_lines(bytes, "ENGINE.txt", HEADER_MARKERS, parse_row)
}

fn parse_row(line: &[u8]) -> Result<EngineRef, String> {
    if line.len() < MIN_LEN {
        return Err(format!("row too short ({} bytes)", line.len()));
    }
    let code = field(line, 1, 5);
    if code.is_empty() {
        return Err("missing engine code".into());
    }
    Ok(EngineRef {
        code,
        mfr: field(line, 7, 16),
        model: field(line, 18, 30),
        type_engine: field(line, 32, 33),
        horsepower: field(line, 35, 39),
        thrust: field(line, 41, 46),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::fixed_width::{padded_record, write_field};

    pub(crate) fn fixture_engine(code: &str, mfr: &str, model: &str) -> Vec<u8> {
        let mut buf = padded_record(47);
        write_field(&mut buf, 1, code);
        write_field(&mut buf, 7, mfr);
        write_field(&mut buf, 18, model);
        write_field(&mut buf, 32, "1");
        write_field(&mut buf, 35, "180");
        buf
    }

    #[test]
    fn parses_engine_with_internal_comma() {
        let parsed = parse_engine(&fixture_engine("41530", "LYCOMING", "IO-360-L2A,A"));
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0].mfr, "LYCOMING");
        assert_eq!(parsed.records[0].model, "IO-360-L2A,A");
        assert_eq!(parsed.records[0].horsepower, "180");
    }
}
