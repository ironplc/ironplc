//! The `$` escapes of character string literals (IEC 61131-3, B.1.2.2).
//!
//! This is the one escape table: the parser decodes a literal's source text
//! with [`decode`], and anything that writes a literal back as source uses
//! [`encode`]. See `specs/design/string-literals.md`.
//!
//! Both widths accept `$$`, `$L`, `$N`, `$P`, `$R` and `$T`, in either case.
//! A single-byte (`STRING`) literal spells its delimiter `$'` and any
//! character as `$` and two hex digits; a double-byte (`WSTRING`) literal
//! spells its delimiter `$"` and any character as `$` and four hex digits.
//! The other width's delimiter may be escaped too and stands for itself.
//! `$N` (newline) is a line feed, the same character as `$L`: the standard
//! leaves the newline character to the implementation.

use core::ops::Range;

use crate::common::StringType;

/// The characters of a literal, and where its source text has an escape the
/// standard does not define.
#[derive(Debug, PartialEq)]
pub struct Decoded {
    /// The characters the literal denotes. An invalid escape is kept as
    /// written.
    pub chars: Vec<char>,
    /// The byte range, in the text given to [`decode`], of each invalid
    /// escape: the `$` and the characters after it that were read with it.
    pub invalid: Vec<Range<usize>>,
}

/// Decodes the text between a literal's delimiters.
pub fn decode(text: &str, width: &StringType) -> Decoded {
    let digits = hex_digit_count(width);
    let mut chars = Vec::with_capacity(text.len());
    let mut invalid = Vec::new();
    let mut rest = text.char_indices().peekable();

    while let Some((start, ch)) = rest.next() {
        if ch != '$' {
            chars.push(ch);
            continue;
        }
        let Some(&(_, next)) = rest.peek() else {
            invalid.push(start..text.len());
            chars.push('$');
            break;
        };
        if let Some(named) = named_escape(next) {
            rest.next();
            chars.push(named);
            continue;
        }
        let hex: String = text[start + 1..].chars().take(digits).collect();
        let code = (hex.len() == digits && hex.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(&hex, 16).ok())
            .flatten()
            .and_then(char::from_u32);
        match code {
            Some(decoded) => {
                for _ in 0..digits {
                    rest.next();
                }
                chars.push(decoded);
            }
            None => {
                invalid.push(start..start + 1 + next.len_utf8());
                chars.push('$');
            }
        }
    }

    Decoded { chars, invalid }
}

/// Encodes characters as the text between a literal's delimiters, so that
/// [`decode`] gives the same characters back.
pub fn encode(chars: &[char], width: &StringType) -> String {
    let mut text = String::with_capacity(chars.len());
    for &ch in chars {
        match ch {
            '$' => text.push_str("$$"),
            '\n' => text.push_str("$L"),
            '\r' => text.push_str("$R"),
            '\u{0C}' => text.push_str("$P"),
            '\t' => text.push_str("$T"),
            _ if ch == width.delimiter() => {
                text.push('$');
                text.push(ch);
            }
            _ if ch.is_control() => match width {
                StringType::String => text.push_str(&format!("${:02X}", u32::from(ch))),
                StringType::WString => text.push_str(&format!("${:04X}", u32::from(ch))),
            },
            _ => text.push(ch),
        }
    }
    text
}

/// The number of hex digits that follow `$` in a numeric escape.
fn hex_digit_count(width: &StringType) -> usize {
    match width {
        StringType::String => 2,
        StringType::WString => 4,
    }
}

/// The character a named escape `$c` stands for.
fn named_escape(c: char) -> Option<char> {
    match c {
        '$' => Some('$'),
        '\'' => Some('\''),
        '"' => Some('"'),
        'L' | 'l' | 'N' | 'n' => Some('\n'),
        'P' | 'p' => Some('\u{0C}'),
        'R' | 'r' => Some('\r'),
        'T' | 't' => Some('\t'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    const NARROW: StringType = StringType::String;
    const WIDE: StringType = StringType::WString;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[rstest]
    #[case::plain("abc", "abc")]
    #[case::dollar("costs $$5", "costs $5")]
    #[case::quote("it$'s", "it's")]
    #[case::double_quote("say $\"hi$\"", "say \"hi\"")]
    #[case::line_feed("a$Lb", "a\nb")]
    #[case::newline_is_line_feed("a$nb", "a\nb")]
    #[case::page("$P", "\u{0C}")]
    #[case::carriage_return("$r", "\r")]
    #[case::tab("$T", "\t")]
    #[case::hex("$41$42", "AB")]
    #[case::hex_lower_case("$e9", "é")]
    #[case::raw_double_quote("\"", "\"")]
    fn decode_when_narrow_then_denoted_characters(#[case] text: &str, #[case] expected: &str) {
        let decoded = decode(text, &NARROW);
        assert!(decoded.invalid.is_empty(), "{decoded:?}");
        assert_eq!(chars(expected), decoded.chars);
    }

    #[rstest]
    #[case::hex("$0041$20AC", "A€")]
    #[case::quote("say $\"hi$\"", "say \"hi\"")]
    #[case::single_quote_escaped("it$'s", "it's")]
    #[case::named("$L$T$$", "\n\t$")]
    fn decode_when_wide_then_denoted_characters(#[case] text: &str, #[case] expected: &str) {
        let decoded = decode(text, &WIDE);
        assert!(decoded.invalid.is_empty(), "{decoded:?}");
        assert_eq!(chars(expected), decoded.chars);
    }

    #[rstest]
    #[case::unknown_letter("a$qb", NARROW, 1..3, "a$qb")]
    #[case::one_hex_digit("$4", NARROW, 0..2, "$4")]
    #[case::two_digits_in_wide("$41", WIDE, 0..2, "$41")]
    #[case::trailing_dollar("ab$", NARROW, 2..3, "ab$")]
    #[case::surrogate("$D800", WIDE, 0..2, "$D800")]
    fn decode_when_invalid_escape_then_reported_and_kept(
        #[case] text: &str,
        #[case] width: StringType,
        #[case] range: Range<usize>,
        #[case] kept: &str,
    ) {
        let decoded = decode(text, &width);
        assert_eq!(vec![range], decoded.invalid);
        assert_eq!(chars(kept), decoded.chars);
    }

    #[test]
    fn decode_when_escape_follows_non_ascii_then_range_is_in_bytes() {
        let decoded = decode("é$q", &NARROW);
        assert_eq!(vec![2..4], decoded.invalid);
    }

    #[rstest]
    #[case::dollar("costs $5", NARROW, "costs $$5")]
    #[case::own_delimiter("it's", NARROW, "it$'s")]
    #[case::other_delimiter_is_plain("say \"hi\"", NARROW, "say \"hi\"")]
    #[case::wide_delimiter("say \"hi\"", WIDE, "say $\"hi$\"")]
    #[case::wide_other_delimiter_is_plain("it's", WIDE, "it's")]
    #[case::named_controls("\n\r\u{0C}\t", NARROW, "$L$R$P$T")]
    #[case::other_control_narrow("\u{01}", NARROW, "$01")]
    #[case::other_control_wide("\u{01}", WIDE, "$0001")]
    #[case::non_ascii_as_itself("é€", WIDE, "é€")]
    fn encode_when_characters_then_source_text(
        #[case] value: &str,
        #[case] width: StringType,
        #[case] expected: &str,
    ) {
        assert_eq!(expected, encode(&chars(value), &width));
    }

    #[rstest]
    #[case(NARROW)]
    #[case(WIDE)]
    fn encode_then_decode_when_any_character_then_same_characters(#[case] width: StringType) {
        let value: Vec<char> = (0u32..=0x1FF).filter_map(char::from_u32).collect();
        let decoded = decode(&encode(&value, &width), &width);
        assert!(decoded.invalid.is_empty());
        assert_eq!(value, decoded.chars);
    }
}
