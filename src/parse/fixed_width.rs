use crate::model::{ParseError, ParsedFile};

/// 1-indexed inclusive byte range, decoded as Windows-1252 and trimmed.
pub fn field(line: &[u8], start_1: usize, end_1: usize) -> String {
    let start = start_1.saturating_sub(1);
    if start >= line.len() {
        return String::new();
    }
    let end = end_1.min(line.len());
    encoding_rs::WINDOWS_1252
        .decode(&line[start..end])
        .0
        .trim()
        .trim_matches(',')
        .to_string()
}

pub fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes)
}

pub fn strip_line_ending(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

pub fn looks_like_header(line: &[u8], markers: &[&str]) -> bool {
    let line = strip_bom(line);
    let decoded = encoding_rs::WINDOWS_1252
        .decode(line)
        .0
        .to_ascii_uppercase();
    let trimmed = decoded.trim();
    markers
        .iter()
        .any(|marker| trimmed.starts_with(&marker.to_ascii_uppercase()))
}

/// One BOM / blank / header / row loop for every dump file. Field maps stay per file.
pub fn parse_lines<T>(
    bytes: &[u8],
    file_name: &str,
    header_markers: &[&str],
    parse_row: impl Fn(&[u8]) -> Result<T, String>,
) -> ParsedFile<T> {
    let bytes = strip_bom(bytes);
    let mut out = ParsedFile::default();
    for (idx, raw) in bytes.split(|b| *b == b'\n').enumerate() {
        let line = strip_line_ending(raw);
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        if idx == 0 && looks_like_header(line, header_markers) {
            continue;
        }
        match parse_row(line) {
            Ok(record) => out.records.push(record),
            Err(error) => out.errors.push(ParseError {
                file_name: file_name.into(),
                line_number: idx + 1,
                raw_line: line.to_vec(),
                error,
            }),
        }
    }
    out
}

/// Write `value` into a space-padded buffer at a 1-indexed inclusive start.
/// Used to build fixture records that match FAA byte positions.
pub fn write_field(buf: &mut [u8], start_1: usize, value: &str) {
    let start = start_1.saturating_sub(1);
    let bytes = value.as_bytes();
    if start >= buf.len() {
        return;
    }
    let end = (start + bytes.len()).min(buf.len());
    buf[start..end].copy_from_slice(&bytes[..end - start]);
}

pub fn padded_record(len: usize) -> Vec<u8> {
    vec![b' '; len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_and_trims() {
        let mut buf = padded_record(20);
        write_field(&mut buf, 1, "ABC  ");
        write_field(&mut buf, 7, "XYZ");
        assert_eq!(field(&buf, 1, 5), "ABC");
        assert_eq!(field(&buf, 7, 9), "XYZ");
    }

    #[test]
    fn short_line_returns_partial() {
        assert_eq!(field(b"AB", 1, 5), "AB");
        assert_eq!(field(b"AB", 7, 10), "");
    }

    #[test]
    fn detects_header() {
        assert!(looks_like_header(b"N-NUMBER,SERIAL NUMBER", &["N-NUMBER"]));
        assert!(!looks_like_header(b"12345,172S", &["N-NUMBER"]));
    }

    #[test]
    fn parse_lines_skips_header_and_quarantines_bad_rows() {
        let mut good = padded_record(20);
        write_field(&mut good, 1, "ABC");
        let mut body = b"N-NUMBER\n".to_vec();
        body.extend_from_slice(&good);
        body.push(b'\n');
        body.extend_from_slice(b"\n");
        let parsed = parse_lines(&body, "X.txt", &["N-NUMBER"], |line| {
            let v = field(line, 1, 3);
            if v == "ABC" {
                Ok(v)
            } else {
                Err("nope".into())
            }
        });
        assert_eq!(parsed.records, vec!["ABC".to_string()]);
        assert!(parsed.errors.is_empty());
    }
}
