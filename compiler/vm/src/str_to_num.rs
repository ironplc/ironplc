//! `STRING_TO_<numeric>` under behavior policies (ADR-0049).
//!
//! The conversion's func_id names its target type and both of its policies
//! (see `ironplc_container::builtin::str_to_num`). This module decodes the
//! ID, scans the string under the selected non-numeric policy, and applies the
//! selected failure policy. It holds no policy state: everything it needs is
//! in the func_id it was handed.
//!
//! What counts as a literal is the IEC 61131-3 integer literal grammar, which
//! every surveyed implementation accepts for the string form: decimal digits
//! with optional `_` separators, or a based literal (`2#`, `8#`, `16#`) with
//! the same separators, either preceded by an optional sign. A typed prefix
//! (`UDINT#`) is not accepted. One scanner serves every integer target; the
//! target contributes only its bounds, and a value outside them is a failure
//! rather than a wrap.

use ironplc_container::builtin::str_to_num::{Encoding, Target};
use ironplc_container::policy::{StringToNumFailure, StringToNumNonNumeric};

use crate::error::{StringPreview, Trap};

/// Converts the narrow string `bytes` under `encoding`.
///
/// Returns the value as the slot's `i32` bit pattern, or the trap the failure
/// policy calls for.
pub(crate) fn convert(encoding: Encoding, bytes: &[u8]) -> Result<i32, Trap> {
    // Every target here lives in a 32-bit slot: the scanner has already
    // checked the value against the target's bounds, so the cast keeps the
    // value (sign-extended for a signed target, as the slot convention is)
    // and nothing is truncated.
    let scanned = scan_integer(bytes, encoding.non_numeric, Bounds::of(encoding.target))
        .map(|value| value as i32);
    match (scanned, encoding.failure) {
        (Some(value), _) => Ok(value),
        (None, StringToNumFailure::Zero) => Ok(0),
        (None, StringToNumFailure::Trap) => Err(Trap::StringNotConvertible {
            target: encoding.target,
            value: StringPreview::of(bytes),
        }),
    }
}

/// The values an integer target holds, as the largest magnitude on each side
/// of zero.
///
/// The scanner is one function for every integer target; the target's bounds
/// are the only thing that differs between them. Stated as magnitudes so the
/// check is two unsigned compares against the 64-bit accumulator, and so a
/// 64-bit unsigned target (`u64::MAX`) and a 64-bit signed one (`2^63` below
/// zero) fit without wider arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bounds {
    /// The largest magnitude a negative value may have (`128` for `SINT`,
    /// `0` for an unsigned target).
    pub(crate) below_zero: u64,
    /// The largest value (`127` for `SINT`, `255` for `USINT`).
    pub(crate) above_zero: u64,
}

impl Bounds {
    /// The bounds of `target`.
    pub(crate) fn of(target: Target) -> Bounds {
        match target {
            Target::U32 => Bounds::unsigned(u32::MAX as u64),
            Target::I32 => Bounds::signed(i32::MIN as i64, i32::MAX as u64),
            Target::U8 => Bounds::unsigned(u8::MAX as u64),
            Target::I8 => Bounds::signed(i8::MIN as i64, i8::MAX as u64),
            Target::U16 => Bounds::unsigned(u16::MAX as u64),
            Target::I16 => Bounds::signed(i16::MIN as i64, i16::MAX as u64),
        }
    }

    const fn unsigned(max: u64) -> Bounds {
        Bounds {
            below_zero: 0,
            above_zero: max,
        }
    }

    const fn signed(min: i64, max: u64) -> Bounds {
        Bounds {
            below_zero: min.unsigned_abs(),
            above_zero: max,
        }
    }

    fn contains(self, literal: Literal) -> bool {
        let limit = if literal.negative {
            self.below_zero
        } else {
            self.above_zero
        };
        literal.magnitude <= limit
    }
}

/// Scans `bytes` for an integer within `bounds` under `policy`.
///
/// Returns the value as its two's-complement 64-bit pattern (a signed
/// value sign-extended; an unsigned value zero-extended), which the caller
/// narrows to the slot width. `None` is a failure: no literal where the
/// policy requires one, or a literal whose value is outside `bounds` (out of
/// range is a failure under every policy, never a wrap).
pub(crate) fn scan_integer(
    bytes: &[u8],
    policy: StringToNumNonNumeric,
    bounds: Bounds,
) -> Option<i64> {
    let text = match policy {
        StringToNumNonNumeric::Reject => trim_ascii(bytes),
        StringToNumNonNumeric::IgnoreTrailing => trim_ascii_start(bytes),
        StringToNumNonNumeric::IgnoreSurrounding => skip_to_literal(bytes),
    };
    let (literal, consumed) = leading_literal(text)?;
    if policy == StringToNumNonNumeric::Reject && consumed != text.len() {
        return None;
    }
    let literal = literal?;
    bounds.contains(literal).then(|| literal.value())
}

/// A well-formed literal: its sign and the magnitude it spells.
#[derive(Clone, Copy)]
struct Literal {
    negative: bool,
    magnitude: u64,
}

impl Literal {
    /// The value's 64-bit two's-complement pattern. Only meaningful once
    /// the literal is known to be within its target's bounds: a magnitude
    /// past `i64::MAX` is either an unsigned value whose pattern this is, or
    /// `-2^63`, which `wrapping_neg` produces exactly.
    fn value(self) -> i64 {
        let magnitude = self.magnitude as i64;
        if self.negative {
            magnitude.wrapping_neg()
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
    let mut value: u64 = 0;
    // Overflow is remembered rather than short-circuited: the run's length
    // is needed either way, and the flag keeps the per-digit step free of a
    // branch on the accumulator.
    let mut overflowed = false;
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
        let (shifted, mul_overflowed) = value.overflowing_mul(u64::from(base));
        let (next, add_overflowed) = shifted.overflowing_add(u64::from(digit));
        overflowed |= mul_overflowed | add_overflowed;
        value = next;
        last_was_digit = true;
        len += 1;
    }
    if len == 0 {
        return None;
    }
    Some(((!overflowed).then_some(value), len))
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
        scan_integer(bytes, policy, Bounds::of(Target::U32)).map(|v| v as u32)
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

    // Each target's bounds, checked at both ends and one past each: the
    // scanner is one function, so the bounds are all a target contributes.
    #[rstest]
    #[case::i8(Target::I8, -128, 127)]
    #[case::u8(Target::U8, 0, 255)]
    #[case::i16(Target::I16, -32_768, 32_767)]
    #[case::u16(Target::U16, 0, 65_535)]
    #[case::i32(Target::I32, -2_147_483_648, 2_147_483_647)]
    #[case::u32(Target::U32, 0, 4_294_967_295)]
    fn scan_integer_when_at_or_past_target_bounds_then_in_range_converts_and_past_fails(
        #[case] target: Target,
        #[case] min: i64,
        #[case] max: i64,
    ) {
        let bounds = Bounds::of(target);
        let text = |v: i64| std::format!("{v}").into_bytes();
        assert_eq!(scan_integer(&text(min), Reject, bounds), Some(min));
        assert_eq!(scan_integer(&text(max), Reject, bounds), Some(max));
        assert_eq!(scan_integer(&text(min - 1), Reject, bounds), None);
        assert_eq!(scan_integer(&text(max + 1), Reject, bounds), None);
        assert_eq!(scan_integer(&text(min - 1), IgnoreTrailing, bounds), None);
        assert_eq!(
            scan_integer(&text(max + 1), IgnoreSurrounding, bounds),
            None
        );
    }

    #[test]
    fn scan_integer_when_signed_target_then_negative_literals_convert() {
        let bounds = Bounds::of(Target::I8);
        assert_eq!(scan_integer(b"-5", Reject, bounds), Some(-5));
        assert_eq!(scan_integer(b"-16#80", Reject, bounds), Some(-128));
        assert_eq!(scan_integer(b"a-5;", IgnoreSurrounding, bounds), Some(-5));
        assert_eq!(scan_integer(b"-2#1000_0001", Reject, bounds), None);
    }

    #[test]
    fn scan_integer_when_64_bit_bounds_then_extremes_convert_to_their_bit_patterns() {
        // The bounds the 64-bit targets will use: the accumulator and the
        // value pattern already hold them.
        let unsigned = Bounds::unsigned(u64::MAX);
        assert_eq!(
            scan_integer(b"18446744073709551615", Reject, unsigned),
            Some(-1)
        );
        assert_eq!(
            scan_integer(b"18446744073709551616", Reject, unsigned),
            None
        );
        let signed = Bounds::signed(i64::MIN, i64::MAX as u64);
        assert_eq!(
            scan_integer(b"-9223372036854775808", Reject, signed),
            Some(i64::MIN)
        );
        assert_eq!(scan_integer(b"-9223372036854775809", Reject, signed), None);
        assert_eq!(scan_integer(b"9223372036854775808", Reject, signed), None);
    }

    #[test]
    fn convert_when_signed_target_then_value_sign_extended_in_the_slot() {
        let encoding = Encoding {
            target: Target::I8,
            non_numeric: Reject,
            failure: StringToNumFailure::Trap,
        };
        assert_eq!(convert(encoding, b"-1"), Ok(-1));
        assert_eq!(convert(encoding, b"127"), Ok(127));
        // '300' is out of range: a failure, never 44.
        assert_eq!(
            convert(encoding, b"300"),
            Err(Trap::StringNotConvertible {
                target: Target::I8,
                value: StringPreview::of(b"300"),
            })
        );
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
