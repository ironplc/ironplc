//! Per-code-unit byte width for string types.
//!
//! Per ADR-0016 / ADR-0035, IEC 61131-3 string values have one of two
//! encodings:
//!
//! - `STRING`  — Latin-1, one byte per character
//! - `WSTRING` — UTF-16LE, two bytes per code unit
//!
//! The runtime carries the encoding as part of:
//!
//! 1. each string variable's data-region header
//! 2. each constant-pool string entry
//! 3. each temp-buffer slot in the VM
//!
//! Representing the width as an enum (rather than a `u8`) makes string
//! operations exhaustively match on the two valid encodings and turns
//! boundary reads (bytecode operand, constant-pool tag, data-region header
//! byte) into a single validated conversion point.

use crate::ContainerError;

/// Per-code-unit byte width of a string value.
///
/// The discriminants match the on-disk encoding used by the string header,
/// the constant-pool encoding tag, and the `char_width` operand of
/// `STR_INIT` — so `width as u8` round-trips through the bytecode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CharWidth {
    /// STRING: Latin-1, one byte per character.
    Narrow = 1,
    /// WSTRING: UTF-16LE, two bytes per code unit.
    Wide = 2,
}

impl CharWidth {
    /// Returns the byte width of one code unit.
    #[inline]
    pub const fn byte_width(self) -> u8 {
        self as u8
    }

    /// Returns the byte width as `usize`, for index arithmetic.
    #[inline]
    pub const fn as_usize(self) -> usize {
        self as u8 as usize
    }

    /// Returns `true` for [`CharWidth::Wide`] (UTF-16LE / WSTRING).
    #[inline]
    pub const fn is_wide(self) -> bool {
        matches!(self, CharWidth::Wide)
    }

    /// Parses a `char_width` byte read from a bytecode operand, header field,
    /// or constant-pool tag. Returns
    /// [`ContainerError::InvalidCharWidth`] for any value other than
    /// `1` or `2`.
    pub fn from_u8(value: u8) -> Result<Self, ContainerError> {
        match value {
            1 => Ok(CharWidth::Narrow),
            2 => Ok(CharWidth::Wide),
            _ => Err(ContainerError::InvalidCharWidth(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_u8_when_valid_bytes_then_returns_variant() {
        assert_eq!(CharWidth::from_u8(1).unwrap(), CharWidth::Narrow);
        assert_eq!(CharWidth::from_u8(2).unwrap(), CharWidth::Wide);
    }

    #[test]
    fn from_u8_when_invalid_byte_then_error() {
        assert!(matches!(
            CharWidth::from_u8(0),
            Err(ContainerError::InvalidCharWidth(0))
        ));
        assert!(matches!(
            CharWidth::from_u8(99),
            Err(ContainerError::InvalidCharWidth(99))
        ));
    }

    #[test]
    fn byte_width_when_each_variant_then_matches_discriminant() {
        assert_eq!(CharWidth::Narrow.byte_width(), 1);
        assert_eq!(CharWidth::Wide.byte_width(), 2);
        assert_eq!(CharWidth::Narrow.as_usize(), 1);
        assert_eq!(CharWidth::Wide.as_usize(), 2);
    }

    #[test]
    fn is_wide_when_each_variant_then_matches() {
        assert!(!CharWidth::Narrow.is_wide());
        assert!(CharWidth::Wide.is_wide());
    }
}
