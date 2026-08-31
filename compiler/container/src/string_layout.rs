//! Layout of a string value in the data region.
//!
//! Per ADR-0035 every STRING/WSTRING value in the data region is laid out as
//!
//! ```text
//! [max_length: u16][cur_length: u16][char_width: u16][data: max_length * char_width bytes]
//! ```
//!
//! The `char_width` field carries the per-code-unit byte width (1 for
//! STRING / Latin-1, 2 for WSTRING / UTF-16LE), so the payload of a
//! WSTRING is twice the payload of a STRING of the same declared length.
//!
//! This module is the single definition of that layout. Both the analyzer
//! (sizing slots for string variables and struct fields) and codegen
//! (emitting data-region offsets and strides) size strings through
//! [`string_region_size`] so the two cannot disagree.

use crate::CharWidth;

/// Size in bytes of the string header
/// (`max_length: u16` + `cur_length: u16` + `char_width: u16`).
///
/// See the module documentation for the full layout.
pub const STRING_HEADER_BYTES: usize = 6;

/// Maximum length, in code units, of a STRING/WSTRING declared without an
/// explicit length (IEC 61131-3 implementation-defined default).
pub const DEFAULT_STRING_MAX_LENGTH: u16 = 254;

/// Total bytes a STRING/WSTRING value occupies in the data region: the
/// header plus `max_length * char_width` payload bytes.
///
/// `max_length` is in code units, not bytes — a `WSTRING[10]` has a
/// 10-code-unit maximum length and a 20-byte payload.
pub const fn string_region_size(max_length: u16, char_width: CharWidth) -> u32 {
    STRING_HEADER_BYTES as u32 + (max_length as u32) * (char_width.byte_width() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_region_size_when_narrow_then_header_plus_one_byte_per_code_unit() {
        assert_eq!(string_region_size(10, CharWidth::Narrow), 16);
    }

    #[test]
    fn string_region_size_when_wide_then_header_plus_two_bytes_per_code_unit() {
        assert_eq!(string_region_size(10, CharWidth::Wide), 26);
    }

    #[test]
    fn string_region_size_when_zero_length_then_header_only() {
        assert_eq!(
            string_region_size(0, CharWidth::Narrow),
            STRING_HEADER_BYTES as u32
        );
        assert_eq!(
            string_region_size(0, CharWidth::Wide),
            STRING_HEADER_BYTES as u32
        );
    }

    #[test]
    fn string_region_size_when_max_length_and_wide_then_does_not_overflow() {
        assert_eq!(
            string_region_size(u16::MAX, CharWidth::Wide),
            STRING_HEADER_BYTES as u32 + 2 * u16::MAX as u32
        );
    }

    #[test]
    fn string_region_size_when_default_max_length_then_matches_narrow_layout() {
        assert_eq!(
            string_region_size(DEFAULT_STRING_MAX_LENGTH, CharWidth::Narrow),
            260
        );
    }
}
