//! Rule that every `$` escape in a character string literal is one the
//! standard defines (see `dsl::string_escape`).
//!
//! The parser decodes a literal leniently, keeping an invalid escape as
//! written, so that parsing does not fail on it; this rule is what rejects
//! it, with a span on the escape itself.

use dsl::common::StringType;
use dsl::core::SourceSpan;
use dsl::diagnostic::{Diagnostic, Label};
use dsl::string_escape::decode;
use ironplc_problems::Problem;

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

pub fn apply(tokens: &[Token], _options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();

    for tok in tokens {
        let width = match tok.token_type {
            TokenType::SingleByteString => StringType::String,
            TokenType::DoubleByteString => StringType::WString,
            _ => continue,
        };
        // The token text includes both delimiters, which are one byte each.
        let Some(text) = tok.text.get(1..tok.text.len().saturating_sub(1)) else {
            continue;
        };
        for range in decode(text, &width).invalid {
            let escape = &text[range.clone()];
            errors.push(
                Diagnostic::problem(
                    Problem::InvalidStringEscape,
                    Label::span(
                        SourceSpan {
                            start: tok.span.start + 1 + range.start,
                            end: tok.span.start + 1 + range.end,
                            file_id: tok.span.file_id.clone(),
                        },
                        "Escape",
                    ),
                )
                .with_context("escape", &escape.to_string())
                .with_help(match width {
                    StringType::String => {
                        "Use $$, $', $L, $N, $P, $R, $T, or $ followed by two hex digits."
                    }
                    StringType::WString => {
                        "Use $$, $\", $L, $N, $P, $R, $T, or $ followed by four hex digits."
                    }
                }),
            );
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use dsl::core::FileId;

    use crate::{lexer::tokenize, options::CompilerOptions, rule_token_string_escape::apply};

    fn escape_errors(source: &str) -> Vec<(usize, usize)> {
        let (tokens, diagnostics) = tokenize(source, &FileId::default(), 0, 0);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        match apply(&tokens, &CompilerOptions::default()) {
            Ok(()) => vec![],
            Err(errors) => errors
                .iter()
                .map(|e| (e.primary.location.start, e.primary.location.end))
                .collect(),
        }
    }

    #[test]
    fn apply_when_valid_escapes_then_ok() {
        assert!(escape_errors("x := 'a$$b$L$'$41'; y := \"$0041$\"\";").is_empty());
    }

    #[test]
    fn apply_when_unknown_escape_then_error_on_escape() {
        // `x := 'a$qb';`: the escape `$q` is at bytes 7..9.
        assert_eq!(vec![(7, 9)], escape_errors("x := 'a$qb';"));
    }

    #[test]
    fn apply_when_two_hex_digits_in_wide_string_then_error() {
        assert_eq!(vec![(6, 8)], escape_errors("x := \"$41\";"));
    }
}
