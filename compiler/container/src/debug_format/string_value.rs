//! Reading STRING and WSTRING values out of the data region.
//!
//! A string variable's slot in the variable table is unused: the bytes live in
//! the data region at the offset recorded by its
//! [`StringLayoutEntry`](crate::StringLayoutEntry), laid out per ADR-0035 as
//!
//! ```text
//! [max_length: u16][cur_length: u16][char_width: u16][data…]
//! ```
//!
//! `cur_length` counts code units, so the payload is `cur_length * char_width`
//! bytes — the header's own `char_width` decides both the byte span and the
//! encoding, which is why nothing here needs the variable's IEC type tag.

use std::format;
use std::string::String;

use crate::{CharWidth, STRING_HEADER_BYTES};

/// Reasons a string variable's bytes could not be read from the data region.
///
/// Distinguished (rather than collapsed to a sentinel string) so a caller can
/// mark the value invalid and render it differently from real string content
/// that happens to read like `'<invalid>'`.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum StringReadError {
    /// The recorded `data_offset` plus the string header would read past the
    /// end of the data region.
    OffsetOutOfBounds,
    /// The header was readable but `cur_length * char_width` would read past
    /// the end of the data region.
    LengthOutOfBounds,
    /// The header's `char_width` field is neither 1 nor 2.
    InvalidCharWidth,
}

/// Reads a string value from the data region at `data_offset` and renders it
/// as an IEC 61131-3 literal: single-quoted for a narrow (Latin-1) string,
/// double-quoted for a wide (UTF-16LE) one.
pub(super) fn read_string_value(
    data_region: &[u8],
    data_offset: u32,
) -> Result<String, StringReadError> {
    let off = data_offset as usize;
    if off.saturating_add(STRING_HEADER_BYTES) > data_region.len() {
        return Err(StringReadError::OffsetOutOfBounds);
    }
    let cur_len = u16::from_le_bytes([data_region[off + 2], data_region[off + 3]]) as usize;
    let width_field = u16::from_le_bytes([data_region[off + 4], data_region[off + 5]]);
    let char_width = u8::try_from(width_field)
        .ok()
        .and_then(|w| CharWidth::from_u8(w).ok())
        .ok_or(StringReadError::InvalidCharWidth)?;

    let start = off + STRING_HEADER_BYTES;
    let end = start
        .checked_add(cur_len * char_width.as_usize())
        .ok_or(StringReadError::LengthOutOfBounds)?;
    if end > data_region.len() {
        return Err(StringReadError::LengthOutOfBounds);
    }
    let bytes = &data_region[start..end];

    Ok(match char_width {
        CharWidth::Narrow => format_narrow_literal(bytes),
        CharWidth::Wide => format_wide_literal(bytes),
    })
}

/// Renders Latin-1 STRING bytes as a single-quoted IEC 61131-3 literal.
///
/// Each byte is either passed through as printable ASCII, replaced with one of
/// the named `$`-escapes (`$T`, `$L`, `$P`, `$R`, `$$`, `$'`), or emitted as a
/// `$XX` two-digit hex escape. Bytes above `0x7E` are hex-escaped rather than
/// passed through: they are legal Latin-1, but a terminal or a web page reading
/// the output as UTF-8 would render them as replacement characters.
fn format_narrow_literal(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 2);
    out.push('\'');
    for &b in bytes {
        match b {
            b'$' => out.push_str("$$"),
            b'\'' => out.push_str("$'"),
            0x09 => out.push_str("$T"),
            0x0A => out.push_str("$L"),
            0x0C => out.push_str("$P"),
            0x0D => out.push_str("$R"),
            0x20..=0x7E => out.push(b as char),
            _ => out.push_str(&format!("${b:02X}")),
        }
    }
    out.push('\'');
    out
}

/// Renders UTF-16LE WSTRING bytes as a double-quoted IEC 61131-3 literal.
///
/// The wide form of the same escape rules, with `$"` in place of `$'` and the
/// four-hex-digit `$XXXX` escape IEC defines for double-quoted strings. Each
/// code unit is escaped on its own, so an unpaired surrogate renders as its own
/// escape instead of being lost to a replacement character.
///
/// A trailing odd byte (only reachable from a corrupt data region, since the
/// payload is `cur_length * 2` bytes) is dropped.
fn format_wide_literal(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 2);
    out.push('"');
    for pair in bytes.as_chunks::<2>().0 {
        let unit = u16::from_le_bytes(*pair);
        match unit {
            0x24 => out.push_str("$$"),
            0x22 => out.push_str("$\""),
            0x09 => out.push_str("$T"),
            0x0A => out.push_str("$L"),
            0x0C => out.push_str("$P"),
            0x0D => out.push_str("$R"),
            0x20..=0x7E => out.push(unit as u8 as char),
            _ => out.push_str(&format!("${unit:04X}")),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    /// Builds a data region holding one string value at offset 0.
    fn region(cur_len: u16, char_width: u16, payload: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&64u16.to_le_bytes());
        data.extend_from_slice(&cur_len.to_le_bytes());
        data.extend_from_slice(&char_width.to_le_bytes());
        data.extend_from_slice(payload);
        data
    }

    /// Encodes `text` as the UTF-16LE payload of a wide string.
    fn utf16(text: &str) -> (u16, Vec<u8>) {
        let units: Vec<u16> = text.encode_utf16().collect();
        let bytes = units.iter().flat_map(|u| u.to_le_bytes()).collect();
        (units.len() as u16, bytes)
    }

    #[test]
    fn read_string_value_when_narrow_then_single_quoted_literal() {
        let data = region(5, 1, b"hello_and_more");
        assert_eq!(read_string_value(&data, 0).unwrap(), "'hello'");
    }

    #[test]
    fn read_string_value_when_zero_length_then_empty_quotes() {
        let data = region(0, 1, b"");
        assert_eq!(read_string_value(&data, 0).unwrap(), "''");
    }

    #[test]
    fn read_string_value_when_wide_then_double_quoted_literal() {
        let (len, payload) = utf16("hello");
        let data = region(len, 2, &payload);
        assert_eq!(read_string_value(&data, 0).unwrap(), "\"hello\"");
    }

    /// A wide string's payload is `cur_length * 2` bytes. Reading `cur_length`
    /// bytes instead would truncate `hello` to `he\0`.
    #[test]
    fn read_string_value_when_wide_then_spans_two_bytes_per_code_unit() {
        let (len, payload) = utf16("hi");
        let data = region(len, 2, &payload);
        assert_eq!(read_string_value(&data, 0).unwrap(), "\"hi\"");
        assert_eq!(payload.len(), 4);
    }

    #[test]
    fn read_string_value_when_wide_and_non_ascii_then_four_digit_hex_escape() {
        let (len, payload) = utf16("¤é");
        let data = region(len, 2, &payload);
        assert_eq!(read_string_value(&data, 0).unwrap(), "\"$00A4$00E9\"");
    }

    #[test]
    fn read_string_value_when_offset_beyond_region_then_offset_error() {
        let data = vec![0u8; 4];
        assert_eq!(
            read_string_value(&data, 8),
            Err(StringReadError::OffsetOutOfBounds)
        );
    }

    #[test]
    fn read_string_value_when_cur_len_overruns_then_length_error() {
        let data = region(100, 1, b"short");
        assert_eq!(
            read_string_value(&data, 0),
            Err(StringReadError::LengthOutOfBounds)
        );
    }

    #[test]
    fn read_string_value_when_wide_cur_len_overruns_then_length_error() {
        // Four payload bytes hold two code units, not four.
        let data = region(4, 2, b"hi\0\0");
        assert_eq!(
            read_string_value(&data, 0),
            Err(StringReadError::LengthOutOfBounds)
        );
    }

    #[test]
    fn read_string_value_when_char_width_not_one_or_two_then_char_width_error() {
        let data = region(2, 4, b"hello");
        assert_eq!(
            read_string_value(&data, 0),
            Err(StringReadError::InvalidCharWidth)
        );
    }

    #[test]
    fn read_string_value_when_named_escapes_then_iec_form() {
        let data = region(9, 1, b"a\tb\nc\rd\x0Ce");
        assert_eq!(read_string_value(&data, 0).unwrap(), "'a$Tb$Lc$Rd$Pe'");
    }

    #[test]
    fn read_string_value_when_dollar_or_quote_then_doubled() {
        let data = region(10, 1, b"$1.50 'hi'");
        assert_eq!(read_string_value(&data, 0).unwrap(), "'$$1.50 $'hi$''");
    }

    #[test]
    fn read_string_value_when_wide_dollar_or_quote_then_doubled() {
        let (len, payload) = utf16("$a\"b");
        let data = region(len, 2, &payload);
        assert_eq!(read_string_value(&data, 0).unwrap(), "\"$$a$\"b\"");
    }

    #[test]
    fn read_string_value_when_null_or_high_byte_then_hex_escape() {
        let data = region(3, 1, &[0x00, 0x01, 0xFF]);
        assert_eq!(read_string_value(&data, 0).unwrap(), "'$00$01$FF'");
    }

    #[test]
    fn read_string_value_when_offset_into_region_then_reads_that_value() {
        let mut data = vec![0xAAu8; 8];
        data.extend(region(3, 1, b"abc"));
        assert_eq!(read_string_value(&data, 8).unwrap(), "'abc'");
    }
}
