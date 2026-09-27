use dsl::core::SourceSpan;

use crate::token::{Token, TokenType};

/// Splits the unit suffixes of a duration literal into tokens of their own.
///
/// An identifier may contain digits and `_`, so in `T#1m30s` the lexer reads
/// `1` and then the single identifier `m30s`: every unit after the first is
/// glued to the number that follows it. Inside a duration literal -- a `T`,
/// `TIME` or `LTIME` prefix, `#`, an optional `-`, then the tokens adjacent
/// to it -- such an identifier is split into its runs of letters, of digits
/// and of `_`, each a token with its own span: `m`, `30`, `s`. The grammar
/// then sees one `number unit` pair per part (see
/// `specs/design/time-literals.md`, REQ-TL-021 and REQ-TL-022).
///
/// Only the tokens of a duration literal are touched, so an identifier such
/// as `m30s` anywhere else is unchanged.
pub fn apply(tokens: Vec<Token>) -> Vec<Token> {
    let mut output: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();

    while let Some(tok) = iter.next() {
        let is_prefix = is_duration_prefix(&tok);
        output.push(tok);
        if !is_prefix || !iter.peek().is_some_and(|t| t.token_type == TokenType::Hash) {
            continue;
        }
        let Some(hash) = iter.next() else {
            continue;
        };
        let mut end = hash.span.end;
        output.push(hash);
        if let Some(minus) =
            iter.next_if(|t| t.token_type == TokenType::Minus && t.span.start == end)
        {
            end = minus.span.end;
            output.push(minus);
        }
        while let Some(part) = iter.next_if(|t| is_interval_token(t) && t.span.start == end) {
            end = part.span.end;
            match part.token_type {
                TokenType::Identifier => output.extend(split(part)),
                // `m1.5s` lexes as `m1`, `.`, `5` and `s`: the `.` joins the
                // digits either side of it into the fixed-point value of the
                // last part.
                TokenType::Period => {
                    let joined =
                        output
                            .pop_if(|last| last.token_type == TokenType::Digits)
                            .zip(iter.next_if(|t| {
                                t.token_type == TokenType::Digits && t.span.start == end
                            }));
                    match joined {
                        Some((whole, fraction)) => {
                            end = fraction.span.end;
                            output.push(fixed_point(whole, &part, fraction));
                        }
                        None => output.push(part),
                    }
                }
                _ => output.push(part),
            }
        }
    }

    output
}

/// The fixed-point token for `whole`, `.` and `fraction`, three adjacent
/// tokens.
fn fixed_point(whole: Token, period: &Token, fraction: Token) -> Token {
    Token {
        token_type: TokenType::FixedPoint,
        span: SourceSpan {
            start: whole.span.start,
            end: fraction.span.end,
            file_id: whole.span.file_id.clone(),
        },
        line: whole.line,
        col: whole.col,
        text: format!("{}{}{}", whole.text, period.text, fraction.text),
    }
}

fn is_duration_prefix(tok: &Token) -> bool {
    match tok.token_type {
        TokenType::Time | TokenType::Ltime => true,
        TokenType::Identifier => tok.text.eq_ignore_ascii_case("T"),
        _ => false,
    }
}

fn is_interval_token(tok: &Token) -> bool {
    matches!(
        tok.token_type,
        TokenType::Digits | TokenType::FixedPoint | TokenType::Identifier | TokenType::Period
    )
}

/// The class of a character, for splitting an identifier into runs.
#[derive(PartialEq, Clone, Copy)]
enum Run {
    Letter,
    Digit,
    Underscore,
}

fn run_of(c: char) -> Run {
    if c.is_ascii_digit() {
        Run::Digit
    } else if c == '_' {
        Run::Underscore
    } else {
        Run::Letter
    }
}

/// Splits an identifier into its runs of letters, digits and `_`. Each `_`
/// is a token of its own, as the grammar reads it as a separator.
fn split(tok: Token) -> Vec<Token> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = tok.text.char_indices().collect();
    for (i, &(_, c)) in chars.iter().enumerate() {
        let next = chars.get(i + 1);
        let ends_run = match next {
            None => true,
            Some(&(_, n)) => run_of(c) == Run::Underscore || run_of(n) != run_of(c),
        };
        if ends_run {
            let end = next.map_or(tok.text.len(), |&(o, _)| o);
            pieces.push((start, end, run_of(c)));
            start = end;
        }
    }

    if pieces.len() == 1 {
        return vec![tok];
    }

    pieces
        .into_iter()
        .map(|(start, end, run)| Token {
            token_type: match run {
                Run::Digit => TokenType::Digits,
                Run::Letter | Run::Underscore => TokenType::Identifier,
            },
            span: SourceSpan {
                start: tok.span.start + start,
                end: tok.span.start + end,
                file_id: tok.span.file_id.clone(),
            },
            line: tok.line,
            col: tok.col + start,
            text: tok.text[start..end].to_string(),
        })
        .collect()
}

#[cfg(test)]
mod test {
    use dsl::core::FileId;

    use crate::lexer::tokenize;
    use crate::token::TokenType;

    use super::apply;

    fn texts(source: &str) -> Vec<String> {
        let (tokens, diagnostics) = tokenize(source, &FileId::default(), 0, 0);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        apply(tokens)
            .into_iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .map(|t| t.text)
            .collect()
    }

    #[test]
    fn apply_when_compound_duration_then_units_split() {
        assert_eq!(
            vec!["T", "#", "1", "d", "2", "h", "30", "m", "15", "s", "500", "ms"],
            texts("T#1d2h30m15s500ms")
        );
    }

    #[test]
    fn apply_when_underscore_between_parts_then_separate_token() {
        assert_eq!(
            vec!["TIME", "#", "1", "h", "_", "30", "m"],
            texts("TIME#1h_30m")
        );
    }

    #[test]
    fn apply_when_negative_duration_then_units_split() {
        assert_eq!(vec!["t", "#", "-", "1", "m", "30", "s"], texts("t#-1m30s"));
    }

    #[test]
    fn apply_when_split_then_spans_follow_source() {
        let (tokens, _) = tokenize("T#1m30s", &FileId::default(), 0, 0);
        let spans: Vec<(usize, usize)> = apply(tokens)
            .iter()
            .map(|t| (t.span.start, t.span.end))
            .collect();
        assert_eq!(vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 6), (6, 7)], spans);
    }

    #[test]
    fn apply_when_fixed_point_after_unit_then_one_fixed_point_token() {
        assert_eq!(vec!["T", "#", "1", "m", "1.5", "s"], texts("T#1m1.5s"));
    }

    #[test]
    fn apply_when_identifier_outside_duration_then_unchanged() {
        assert_eq!(vec!["x", ":=", "m30s", ";"], texts("x := m30s;"));
    }

    #[test]
    fn apply_when_literal_ends_then_next_token_unchanged() {
        assert_eq!(vec!["T", "#", "5", "s", "+", "a1b"], texts("T#5s + a1b"));
    }
}
