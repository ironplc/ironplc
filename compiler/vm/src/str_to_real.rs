//! `STRING_TO_REAL` and `STRING_TO_LREAL` under behavior policies
//! (ADR-0049): the real-number scanner.
//!
//! The non-numeric policy is applied exactly as for the integer targets,
//! with the same trim and skip helpers, so `'12abc'` and `'abc12'` mean the
//! same thing whatever the target. What differs is the literal: an optional
//! sign, a mantissa (digits, digits with a decimal point and an optional
//! fraction, or a decimal point with a fraction), and an optional exponent
//! (`E` or `e`, an optional sign, digits), with single `_` separators
//! between the digits of a run. `inf` and `nan` are words, not literals, so
//! nothing here produces an infinity or a NaN.
//!
//! The literal is measured here and parsed by `core`'s `FromStr` at the
//! target's width, so the value is the correctly rounded value of the text
//! at that width. A magnitude that rounds to infinity does not fit the
//! target and is a failure, like an integer past its bounds.

use ironplc_container::policy::StringToNumNonNumeric;

use crate::str_to_num::{next_is_digit, trim_ascii, trim_ascii_start};

/// Scans `bytes` for a `REAL` under `policy`.
pub(crate) fn scan_f32(bytes: &[u8], policy: StringToNumNonNumeric) -> Option<f32> {
    scan(bytes, policy, |text| {
        text.parse::<f32>().ok().filter(|value| value.is_finite())
    })
}

/// Scans `bytes` for an `LREAL` under `policy`.
pub(crate) fn scan_f64(bytes: &[u8], policy: StringToNumNonNumeric) -> Option<f64> {
    scan(bytes, policy, |text| {
        text.parse::<f64>().ok().filter(|value| value.is_finite())
    })
}

/// The longest literal, less `_` separators, that a parse buffer holds.
///
/// A literal longer than this with separators in it is a failure. No real
/// number needs it: the widest target has seventeen significant digits and
/// a three-digit exponent.
const PARSE_BUFFER: usize = 128;

fn scan<T>(
    bytes: &[u8],
    policy: StringToNumNonNumeric,
    parse: impl Fn(&str) -> Option<T>,
) -> Option<T> {
    let text = match policy {
        StringToNumNonNumeric::Reject => trim_ascii(bytes),
        StringToNumNonNumeric::IgnoreTrailing => trim_ascii_start(bytes),
        StringToNumNonNumeric::IgnoreSurrounding => skip_to_literal(bytes),
    };
    let (len, separated) = leading_literal(text)?;
    if policy == StringToNumNonNumeric::Reject && len != text.len() {
        return None;
    }
    let literal = &text[..len];
    // The grammar admits only ASCII, so the bytes are a `str` as they are;
    // only the separators keep `FromStr` from reading them directly.
    if !separated {
        return parse(core::str::from_utf8(literal).ok()?);
    }
    let mut buffer = [0u8; PARSE_BUFFER];
    let mut written = 0;
    for &byte in literal.iter().filter(|&&b| b != b'_') {
        *buffer.get_mut(written)? = byte;
        written += 1;
    }
    parse(core::str::from_utf8(&buffer[..written]).ok()?)
}

/// The length of the longest leading run of `text` that is a real literal
/// and whether it carries a `_` separator, or `None` when `text` does not
/// start with one.
fn leading_literal(text: &[u8]) -> Option<(usize, bool)> {
    let mut pos = 0;
    if matches!(text.first(), Some(b'+') | Some(b'-')) {
        pos = 1;
    }

    let integer = digit_run(&text[pos..]);
    pos += integer.len;
    let mut separated = integer.separated;
    if text.get(pos) == Some(&b'.') {
        let fraction = digit_run(&text[pos + 1..]);
        if integer.len == 0 && fraction.len == 0 {
            return None;
        }
        pos += 1 + fraction.len;
        separated |= fraction.separated;
    } else if integer.len == 0 {
        return None;
    }

    // An exponent marker is part of the literal only with digits behind it;
    // `'1e'` is the literal `1` with `e` trailing.
    if matches!(text.get(pos), Some(b'E') | Some(b'e')) {
        let mut exponent_pos = pos + 1;
        if matches!(text.get(exponent_pos), Some(b'+') | Some(b'-')) {
            exponent_pos += 1;
        }
        let exponent = digit_run(&text[exponent_pos..]);
        if exponent.len > 0 {
            pos = exponent_pos + exponent.len;
            separated |= exponent.separated;
        }
    }
    Some((pos, separated))
}

/// The longest leading run of decimal digits in a text, with single `_`
/// separators between digits.
struct DigitRun {
    /// Its length, zero when the text does not start with a digit.
    len: usize,
    /// Whether it carries a separator.
    separated: bool,
}

fn digit_run(text: &[u8]) -> DigitRun {
    let mut run = DigitRun {
        len: 0,
        separated: false,
    };
    let mut last_was_digit = false;
    for &byte in text {
        if byte.is_ascii_digit() {
            last_was_digit = true;
        } else if byte == b'_' && last_was_digit && next_is_digit(text, run.len + 1, 10) {
            last_was_digit = false;
            run.separated = true;
        } else {
            break;
        }
        run.len += 1;
    }
    run
}

/// Skips leading bytes that cannot start a real literal: everything up to
/// the first digit, a decimal point followed by a digit, or a sign followed
/// by either.
fn skip_to_literal(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() {
        if can_start_literal(&bytes[start..]) {
            break;
        }
        start += 1;
    }
    &bytes[start..]
}

fn can_start_literal(text: &[u8]) -> bool {
    let unsigned = match text.first() {
        Some(b'+') | Some(b'-') => &text[1..],
        _ => text,
    };
    match unsigned.first() {
        Some(b) if b.is_ascii_digit() => true,
        Some(b'.') => next_is_digit(unsigned, 1, 10),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use StringToNumNonNumeric::{IgnoreSurrounding, IgnoreTrailing, Reject};

    // Inputs that are a whole literal convert identically under every
    // policy, at both widths.
    #[rstest]
    #[case::decimal_point(b"9.876", 9.876)]
    #[case::exponent(b"1.2E-34", 1.2e-34)]
    #[case::lower_exponent(b"1.5e3", 1500.0)]
    #[case::exponent_plus(b"+2E+2", 200.0)]
    #[case::integer_form(b"5", 5.0)]
    #[case::point_last(b"1.", 1.0)]
    #[case::point_first(b".5", 0.5)]
    #[case::negative(b"-0.5", -0.5)]
    #[case::negative_point_first(b"-.5", -0.5)]
    #[case::underscores(b"1_000.000_5", 1000.0005)]
    #[case::underscore_in_exponent(b"1e1_0", 1e10)]
    #[case::surrounding_whitespace(b" \t2.5\r\n", 2.5)]
    #[case::leading_zeros(b"007.50", 7.5)]
    fn scan_when_whole_literal_then_value_under_every_policy(
        #[case] input: &[u8],
        #[case] expected: f64,
    ) {
        for policy in [Reject, IgnoreTrailing, IgnoreSurrounding] {
            assert_eq!(scan_f64(input, policy), Some(expected), "{policy:?}");
            assert_eq!(scan_f32(input, policy), Some(expected as f32), "{policy:?}");
        }
    }

    // Inputs no policy converts: nothing numeric, a word that would spell
    // an infinity or a NaN, or a magnitude that rounds to infinity.
    #[rstest]
    #[case::empty(b"")]
    #[case::whitespace_only(b"  ")]
    #[case::letters(b"abc")]
    #[case::sign_only(b"-")]
    #[case::point_only(b".")]
    #[case::sign_and_point(b"-.")]
    #[case::inf(b"inf")]
    #[case::infinity(b"-infinity")]
    #[case::nan(b"NaN")]
    #[case::overflow_at_both_widths(b"1e400")]
    fn scan_when_never_convertible_then_none_under_every_policy(#[case] input: &[u8]) {
        for policy in [Reject, IgnoreTrailing, IgnoreSurrounding] {
            assert_eq!(scan_f64(input, policy), None, "{policy:?}");
            assert_eq!(scan_f32(input, policy), None, "{policy:?}");
        }
    }

    #[test]
    fn scan_when_magnitude_rounds_to_infinity_at_the_width_then_none() {
        // 1e39 is past f32 but within f64: a failure only for REAL.
        assert_eq!(scan_f32(b"1e39", Reject), None);
        assert_eq!(scan_f64(b"1e39", Reject), Some(1e39));
        assert_eq!(scan_f64(b"1e309", Reject), None);
        assert_eq!(scan_f32(b"-3.5e38", Reject), None);
        assert_eq!(scan_f32(b"3.4e38", Reject), Some(3.4e38));
    }

    #[test]
    fn scan_when_magnitude_underflows_then_zero_or_subnormal_not_a_failure() {
        assert_eq!(scan_f32(b"1e-50", Reject), Some(0.0));
        assert_eq!(scan_f64(b"1e-50", Reject), Some(1e-50));
        assert_eq!(scan_f64(b"1e-400", Reject), Some(0.0));
        assert!(scan_f32(b"1e-40", Reject).unwrap().is_subnormal());
    }

    #[test]
    fn scan_when_negative_zero_then_negative_zero() {
        assert!(scan_f64(b"-0.0", Reject).unwrap().is_sign_negative());
        assert_eq!(scan_f64(b"-0.0", Reject), Some(0.0));
    }

    #[test]
    fn scan_when_value_is_parsed_at_the_width_then_correctly_rounded_not_narrowed() {
        // 1.0000000596046448 lies exactly halfway between two f32 values;
        // parsing at f32 rounds the decimal text directly, and must agree
        // with the f32 parse of the same text rather than with narrowing
        // an f64.
        let text = "1.00000005960464477539062500000001";
        let direct = text.parse::<f32>().unwrap();
        let narrowed = text.parse::<f64>().unwrap() as f32;
        assert_ne!(direct, narrowed);
        assert_eq!(scan_f32(text.as_bytes(), Reject), Some(direct));
    }

    // Trailing characters: rejected, or ignored after the leading literal.
    #[rstest]
    #[case::letters_after(b"12abc", 12.0)]
    #[case::second_point(b"1.5.5", 1.5)]
    #[case::exponent_without_digits(b"1e", 1.0)]
    #[case::exponent_sign_without_digits(b"1e-", 1.0)]
    #[case::exponent_with_point(b"1e5.5", 1e5)]
    #[case::comma(b"12,5", 12.0)]
    #[case::based_literal_is_not_real(b"16#FF", 16.0)]
    #[case::trailing_underscore(b"1_", 1.0)]
    #[case::inf_after_number(b"1inf", 1.0)]
    fn scan_when_trailing_characters_then_reject_fails_and_others_take_prefix(
        #[case] input: &[u8],
        #[case] prefix: f64,
    ) {
        assert_eq!(scan_f64(input, Reject), None);
        assert_eq!(scan_f64(input, IgnoreTrailing), Some(prefix));
        assert_eq!(scan_f64(input, IgnoreSurrounding), Some(prefix));
    }

    // Leading characters: only `ignore-surrounding` skips them.
    #[rstest]
    #[case::letters_before(b"abc1.5", 1.5)]
    #[case::units(b"x=2.5;", 2.5)]
    #[case::nan_then_digit(b"nan5", 5.0)]
    #[case::exponent_marker_then_digit(b"e5", 5.0)]
    #[case::sign_kept_with_number(b"a-1.5", -1.5)]
    #[case::sign_not_before_number(b"a-b5", 5.0)]
    #[case::point_kept_with_fraction(b"v.5", 0.5)]
    #[case::point_not_before_digit(b".x5", 5.0)]
    #[case::typed_prefix(b"REAL#2.5", 2.5)]
    fn scan_when_leading_characters_then_only_ignore_surrounding_converts(
        #[case] input: &[u8],
        #[case] expected: f64,
    ) {
        assert_eq!(scan_f64(input, Reject), None);
        assert_eq!(scan_f64(input, IgnoreTrailing), None);
        assert_eq!(scan_f64(input, IgnoreSurrounding), Some(expected));
    }

    #[test]
    fn scan_when_prefix_overflows_then_range_failure_not_shorter_prefix() {
        // The leading literal is the whole run; it does not fit REAL, so
        // the conversion fails rather than converting a shorter run.
        assert_eq!(scan_f32(b"1e39abc", IgnoreTrailing), None);
    }

    #[test]
    fn scan_when_separators_exceed_the_parse_buffer_then_none() {
        let mut long = [b'1'; PARSE_BUFFER + 2];
        long[1] = b'_';
        long[PARSE_BUFFER + 1] = b'0';
        assert_eq!(scan_f64(&long, Reject), None);
        // Without separators the same length parses directly.
        let long = [b'1'; PARSE_BUFFER + 2];
        assert!(scan_f64(&long, Reject).is_some());
    }

    #[test]
    fn scan_when_non_ascii_bytes_then_treated_as_non_numeric() {
        assert_eq!(scan_f64(b"1.5\xE9", Reject), None);
        assert_eq!(scan_f64(b"1.5\xE9", IgnoreTrailing), Some(1.5));
        assert_eq!(scan_f64(b"\xE91.5", IgnoreSurrounding), Some(1.5));
    }
}
