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
}
