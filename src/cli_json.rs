//! Minimal JSON output helpers for the qzt CLI (RFC 8259 string escaping).
//! The qzt crate intentionally has no serde dependency; CLI JSON output is
//! flat and hand-assembled through these helpers.

use std::fmt::Write as _;

/// Escapes a string for use inside a JSON string literal.
///
/// Produces the bare content (without the surrounding `"` delimiters).
pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Formats a byte slice as lowercase hexadecimal (no separators).
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_plain_string_unchanged() {
        assert_eq!(escape("hello"), "hello");
    }

    #[test]
    fn escape_quote_and_backslash() {
        assert_eq!(escape("a\"b\\c\nd"), r#"a\"b\\c\nd"#);
    }

    #[test]
    fn escape_control_character() {
        // U+0001 must appear as 
        assert_eq!(escape("\u{1}"), "\\u0001");
    }

    #[test]
    fn escape_tab_cr_lf() {
        assert_eq!(escape("\t"), "\\t");
        assert_eq!(escape("\r"), "\\r");
        assert_eq!(escape("\n"), "\\n");
    }

    #[test]
    fn hex_two_bytes() {
        assert_eq!(hex(&[0x00, 0xff]), "00ff");
    }

    #[test]
    fn hex_empty() {
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn hex_single_byte() {
        assert_eq!(hex(&[0xab]), "ab");
    }
}
