pub mod dates;
pub mod db;
pub mod download;
pub mod model;
pub mod parse;

pub use dates::{
    faa_date, is_utc_date, is_utc_instant, require_utc_date, require_utc_instant, utc_date, utc_iso,
};

/// Uppercase, strip spaces/dashes/dots, ensure a leading `N`.
pub fn canonical_n_number(raw: &str) -> String {
    let s = raw.trim().to_uppercase().replace(['-', ' ', '.'], "");
    if s.is_empty() {
        return s;
    }
    if s.starts_with('N') {
        s
    } else {
        format!("N{s}")
    }
}

/// Lowercase Mode S hex for joins. Empty if the dump hex is empty.
pub fn canonical_icao24(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// True when `raw` is a 6-char Mode S hex (no leading `N`).
pub fn looks_like_mode_s_hex(raw: &str) -> bool {
    let s = raw.trim();
    if s.len() != 6 {
        return false;
    }
    let upper = s.to_ascii_uppercase();
    if upper.starts_with('N') {
        return false;
    }
    upper.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_n_keeps_leading_n() {
        assert_eq!(canonical_n_number("1WM"), "N1WM");
        assert_eq!(canonical_n_number("n12345"), "N12345");
        assert_eq!(canonical_n_number("  Nabc  "), "NABC");
        assert_eq!(canonical_n_number("98765"), "N98765");
        assert_eq!(canonical_n_number("N-66-CL"), "N66CL");
    }

    #[test]
    fn icao24_is_lowercase() {
        assert_eq!(canonical_icao24("A00B1C"), "a00b1c");
        assert_eq!(canonical_icao24("  "), "");
        assert_eq!(canonical_icao24(""), "");
    }

    #[test]
    fn mode_s_hex_detection() {
        assert!(looks_like_mode_s_hex("A00B1C"));
        assert!(looks_like_mode_s_hex("a8b21d"));
        assert!(!looks_like_mode_s_hex("N66CL"));
        assert!(!looks_like_mode_s_hex("66CL"));
        assert!(!looks_like_mode_s_hex("12345"));
    }
}
