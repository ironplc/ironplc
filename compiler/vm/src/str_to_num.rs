//! `STRING_TO_<numeric>` under behavior policies (ADR-0049).
//!
//! The conversion's func_id names its target type and both of its policies
//! (see `ironplc_container::builtin::str_to_num`). This module decodes the
//! ID, scans the string under the selected non-numeric policy, and applies the
//! selected failure policy. It holds no policy state: everything it needs is
//! in the func_id it was handed.
//!
//! What counts as a literal is the IEC 61131-3 unsigned integer literal
//! grammar, which every surveyed implementation accepts for the string form:
//! decimal digits with optional `_` separators, or a based literal (`2#`,
//! `8#`, `16#`) with the same separators, either preceded by an optional
//! sign. A typed prefix (`UDINT#`) is not accepted.

use ironplc_container::builtin::str_to_num::{Encoding, Target};
use ironplc_container::policy::{StringToNumFailure, StringToNumNonNumeric};

use crate::error::{StringPreview, Trap};

/// Converts the narrow string `bytes` under `encoding`.
///
/// Returns the value as the slot's `i32` bit pattern, or the trap the failure
/// policy calls for.
pub(crate) fn convert(encoding: Encoding, bytes: &[u8]) -> Result<i32, Trap> {
    let scanned = match encoding.target {
        Target::U32 => scan_integer(bytes, encoding.non_numeric, Bounds::U32).map(|v| v as i32),
    };
    match (scanned, encoding.failure) {
        (Some(value), _) => Ok(value),
        (None, StringToNumFailure::Zero) => Ok(0),
        (None, StringToNumFailure::Trap) => Err(Trap::StringNotConvertible {
            target: encoding.target,
            value: StringPreview::of(bytes),
        }),
    }
}

/// The values an integer target holds, inclusive at both ends.
///
/// The scanner is one function for every integer target; the target's bounds
/// are the only thing that differs between them. Wide enough for a 64-bit
/// unsigned target, so the same scanner serves the 64-bit targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bounds {
    pub(crate) min: i128,
    pub(crate) max: i128,
}

impl Bounds {
    /// An unsigned 32-bit target (`UDINT`).
    pub(crate) const U32: Bounds = Bounds {
        min: 0,
        max: u32::MAX as i128,
    };

    fn contains(self, value: i128) -> bool {
        (self.min..=self.max).contains(&value)
    }
}

/// Scans `bytes` for an integer within `bounds` under `policy`.
///
/// `None` is a failure: no literal where the policy requires one, or a
/// literal whose value is outside `bounds` (out of range is a failure under
/// every policy, never a wrap).
pub(crate) fn scan_integer(
    bytes: &[u8],
    policy: StringToNumNonNumeric,
    bounds: Bounds,
) -> Option<i128> {
    let text = match policy {
        StringToNumNonNumeric::Reject => trim_ascii(bytes),
        StringToNumNonNumeric::IgnoreTrailing => trim_ascii_start(bytes),
        StringToNumNonNumeric::IgnoreSurrounding => skip_to_literal(bytes),
    };
    let (literal, consumed) = leading_literal(text)?;
    if policy == StringToNumNonNumeric::Reject && consumed != text.len() {
        return None;
    }
    let value = literal?.value();
    bounds.contains(value).then_some(value)
}

/// A well-formed literal: its sign and the magnitude it spells.
#[derive(Clone, Copy)]
struct Literal {
    negative: bool,
    magnitude: u64,
}

impl Literal {
    fn value(self) -> i128 {
        let magnitude = self.magnitude as i128;
        if self.negative {
            -magnitude
        } else {
            magnitude
        }
    }
}

/// The literal and length of the longest leading run of `text` that is a
/// literal, or `None` when `text` does not start with one.
///
/// The literal is `None` (with the run still measured) when the run is
/// well-formed but its magnitude overflows the 64-bit accumulator: the run is
/// still the literal, so under `ignore-trailing` the rest is still ignored,
/// and the conversion fails on range rather than on syntax.
fn leading_literal(text: &[u8]) -> Option<(Option<Literal>, usize)> {
    let mut pos = 0;
    let negative = match text.first() {
        Some(b'+') => {
            pos = 1;
            false
        }
        Some(b'-') => {
            pos = 1;
            true
        }
        _ => false,
    };

    let (mut magnitude, digits_len) = digit_run(&text[pos..], 10)?;
    pos += digits_len;

    // A decimal run of `2`, `8` or `16` followed by `#` and at least one
    // digit of that base is a based literal. Anything else after the run,
    // `#` included, is not part of the literal.
    if let Some(b'#') = text.get(pos) {
        let base = match magnitude {
            Some(2) => Some(2),
            Some(8) => Some(8),
            Some(16) => Some(16),
            _ => None,
        };
        if let Some(base) = base {
            if let Some((based_magnitude, based_len)) = digit_run(&text[pos + 1..], base) {
                magnitude = based_magnitude;
                pos += 1 + based_len;
            }
        }
    }

    let literal = magnitude.map(|magnitude| Literal {
        negative,
        magnitude,
    });
    Some((literal, pos))
}

/// The value and length of the longest leading run of digits in `base`,
/// with single `_` separators between digits, or `None` when `text` does not
/// start with a digit. The value is `None` when the run overflows `u64`.
fn digit_run(text: &[u8], base: u32) -> Option<(Option<u64>, usize)> {
    let mut value: Option<u64> = Some(0);
    let mut len = 0;
    let mut last_was_digit = false;
    for &byte in text {
        let digit = match (byte as char).to_digit(base) {
            Some(d) => d,
            // An underscore continues the run only between two digits.
            None if byte == b'_' && last_was_digit && next_is_digit(text, len + 1, base) => {
                last_was_digit = false;
                len += 1;
                continue;
            }
            None => break,
        };
        value = value
            .and_then(|v| v.checked_mul(u64::from(base)))
            .and_then(|v| v.checked_add(u64::from(digit)));
        last_was_digit = true;
        len += 1;
    }
    if len == 0 {
        return None;
    }
    Some((value, len))
}

fn next_is_digit(text: &[u8], index: usize, base: u32) -> bool {
    text.get(index).is_some_and(|&b| (b as char).is_digit(base))
}

/// Skips leading bytes that cannot start a literal: everything up to the
/// first digit, or a sign immediately followed by a digit (Rockwell's `STOD`
/// keeps "the minus sign in front of a number").
fn skip_to_literal(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() {
        let b = bytes[start];
        if b.is_ascii_digit() {
            break;
        }
        if (b == b'-' || b == b'+') && next_is_digit(bytes, start + 1, 10) {
            break;
        }
        start += 1;
    }
    &bytes[start..]
}

fn is_ascii_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|&b| !is_ascii_space(b))
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let bytes = trim_ascii_start(bytes);
    let end = bytes
        .iter()
        .rposition(|&b| !is_ascii_space(b))
        .map_or(0, |i| i + 1);
    &bytes[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use StringToNumNonNumeric::{IgnoreSurrounding, IgnoreTrailing, Reject};

    fn scan_u32(bytes: &[u8], policy: StringToNumNonNumeric) -> Option<u32> {
        scan_integer(bytes, policy, Bounds::U32).map(|v| v as u32)
    }

    // Inputs that are a whole literal convert identically under every
    // policy: the policies differ only in what they do with the rest.
    #[rstest]
    #[case::zero(b"0", 0)]
    #[case::plain(b"123", 123)]
    #[case::plus_sign(b"+123", 123)]
    #[case::minus_zero(b"-0", 0)]
    #[case::i32_max(b"2147483647", 2_147_483_647)]
    #[case::above_i32_max(b"2147483648", 2_147_483_648)]
    #[case::u32_max(b"4294967295", u32::MAX)]
    #[case::underscores(b"1_000_000", 1_000_000)]
    #[case::hex(b"16#FF", 255)]
    #[case::hex_lower(b"16#ff", 255)]
    #[case::hex_underscore(b"16#FFFF_FFFF", u32::MAX)]
    #[case::binary(b"2#1010", 10)]
    #[case::octal(b"8#17", 15)]
    #[case::surrounding_whitespace(b"  42\t\r\n", 42)]
    #[case::leading_zeros(b"007", 7)]
    fn scan_u32_when_whole_literal_then_value_under_every_policy(
        #[case] input: &[u8],
        #[case] expected: u32,
    ) {
        assert_eq!(scan_u32(input, Reject), Some(expected));
        assert_eq!(scan_u32(input, IgnoreTrailing), Some(expected));
        assert_eq!(scan_u32(input, IgnoreSurrounding), Some(expected));
    }

    // Inputs no policy converts: nothing numeric at all, or a literal whose
    // value does not fit.
    #[rstest]
    #[case::empty(b"")]
    #[case::whitespace_only(b"   ")]
    #[case::letters(b"abc")]
    #[case::sign_only(b"-")]
    #[case::u32_max_plus_one(b"4294967296")]
    #[case::hex_out_of_range(b"16#1_0000_0000")]
    #[case::negative(b"-1")]
    fn scan_u32_when_never_convertible_then_none_under_every_policy(#[case] input: &[u8]) {
        assert_eq!(scan_u32(input, Reject), None);
        assert_eq!(scan_u32(input, IgnoreTrailing), None);
        assert_eq!(scan_u32(input, IgnoreSurrounding), None);
    }

    // Trailing characters: rejected, or ignored after the leading literal.
    #[rstest]
    #[case::letters_after(b"12abc", 12)]
    #[case::space_inside(b"12 34", 12)]
    #[case::decimal_point(b"12.5", 12)]
    #[case::trailing_underscore(b"12_", 12)]
    #[case::double_underscore(b"1__2", 1)]
    #[case::hash_without_base(b"12#FF", 12)]
    #[case::hash_without_digits(b"16#", 16)]
    #[case::hex_then_letters(b"16#FFxyz", 255)]
    #[case::binary_then_decimal_digit(b"2#1012", 5)]
    fn scan_u32_when_trailing_characters_then_reject_fails_and_others_take_prefix(
        #[case] input: &[u8],
        #[case] prefix: u32,
    ) {
        assert_eq!(scan_u32(input, Reject), None);
        assert_eq!(scan_u32(input, IgnoreTrailing), Some(prefix));
        assert_eq!(scan_u32(input, IgnoreSurrounding), Some(prefix));
    }

    // Leading characters: only `ignore-surrounding` skips them.
    #[rstest]
    #[case::letters_before(b"abc12", 12)]
    #[case::units(b"x=42;", 42)]
    #[case::sign_not_before_digit(b"a-b5", 5)]
    #[case::plus_before_digit(b"v+7", 7)]
    #[case::typed_prefix(b"UDINT#5", 5)]
    #[case::underscore_first(b"_1", 1)]
    fn scan_u32_when_leading_characters_then_only_ignore_surrounding_converts(
        #[case] input: &[u8],
        #[case] expected: u32,
    ) {
        assert_eq!(scan_u32(input, Reject), None);
        assert_eq!(scan_u32(input, IgnoreTrailing), None);
        assert_eq!(scan_u32(input, IgnoreSurrounding), Some(expected));
    }

    #[test]
    fn scan_u32_when_minus_before_digits_under_ignore_surrounding_then_range_failure() {
        // The sign is kept with the number, so this is `-5`, which an
        // unsigned target does not hold: a failure, not a skip to `5`.
        assert_eq!(scan_u32(b"a-5", IgnoreSurrounding), None);
    }

    #[test]
    fn scan_u32_when_prefix_overflows_then_range_failure_not_shorter_prefix() {
        // The leading literal is the whole digit run; it does not fit, so
        // the conversion fails rather than converting a shorter run.
        assert_eq!(scan_u32(b"4294967296abc", IgnoreTrailing), None);
    }

    #[test]
    fn scan_u32_when_non_ascii_bytes_then_treated_as_non_numeric() {
        // Latin-1 bytes above 0x7F are characters, not digits.
        assert_eq!(scan_u32(b"12\xE9", Reject), None);
        assert_eq!(scan_u32(b"12\xE9", IgnoreTrailing), Some(12));
        assert_eq!(scan_u32(b"\xE912", IgnoreSurrounding), Some(12));
    }

    fn u32_encoding(non_numeric: StringToNumNonNumeric, failure: StringToNumFailure) -> Encoding {
        Encoding {
            target: Target::U32,
            non_numeric,
            failure,
        }
    }

    #[test]
    fn convert_when_convertible_then_value_as_slot_bits() {
        let encoding = u32_encoding(Reject, StringToNumFailure::Trap);
        assert_eq!(convert(encoding, b"4294967295"), Ok(-1));
        assert_eq!(convert(encoding, b"2147483648"), Ok(i32::MIN));
    }

    #[test]
    fn convert_when_failure_and_zero_policy_then_zero() {
        let encoding = u32_encoding(Reject, StringToNumFailure::Zero);
        assert_eq!(convert(encoding, b"12abc"), Ok(0));
    }

    #[test]
    fn convert_when_failure_and_trap_policy_then_trap_names_value() {
        let encoding = u32_encoding(Reject, StringToNumFailure::Trap);
        let trap = convert(encoding, b"12abc").unwrap_err();
        assert_eq!(
            trap,
            Trap::StringNotConvertible {
                target: Target::U32,
                value: StringPreview::of(b"12abc"),
            }
        );
        assert_eq!(trap.v_code(), "V4006");
    }
}
