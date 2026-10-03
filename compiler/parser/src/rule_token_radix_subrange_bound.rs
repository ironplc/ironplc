//! Validation rule: reject a subrange bound written in hex, binary or octal
//! (`16#01..16#0F`) unless the flag for its context is set.
//!
//! IEC 61131-3 (Annex B) defines `subrange ::= signed_integer '..'
//! signed_integer`, where `signed_integer` is a decimal digit sequence, so a
//! radix bound is an extension. The grammar accepts it unconditionally (see
//! `subrange_bound()`) and holds it as the same literal a decimal bound is,
//! so this token-stream check is what enforces the flags:
//!
//! - In a `CASE` label, `--allow-bit-string-case-labels` (P4041), the flag
//!   that already gates a radix label such as `16#D012:`.
//! - In a subrange type or an array dimension,
//!   `--allow-radix-subrange-bounds` (P4073).
//!
//! A radix token next to a `..` token (ignoring trivia) is a subrange bound:
//! no other construct puts a radix literal beside `..`. Between `CASE` and
//! `END_CASE` a `..` can only separate the bounds of a label, since a
//! statement never contains one, so the `CASE` nesting depth tells a label
//! bound from a declaration bound.
//!
//! The set of tokens skipped here matches the grammar's whitespace rule
//! `_ = (whitespace() / comment() / pragma())*`, so a bound separated from
//! `..` by a comment is found the same way as one written next to it.

use dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

fn is_trivia(t: &TokenType) -> bool {
    matches!(
        t,
        TokenType::Whitespace | TokenType::Newline | TokenType::Comment | TokenType::Pragma
    )
}

fn is_radix(t: &TokenType) -> bool {
    matches!(
        t,
        TokenType::HexDigits | TokenType::OctDigits | TokenType::BinDigits
    )
}

pub fn apply(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_bit_string_case_labels && options.allow_radix_subrange_bounds {
        return Ok(());
    }

    let significant: Vec<&Token> = tokens
        .iter()
        .filter(|t| !is_trivia(&t.token_type))
        .collect();

    let mut errors = vec![];
    let mut case_depth: usize = 0;
    for (i, tok) in significant.iter().enumerate() {
        match tok.token_type {
            TokenType::Case => case_depth += 1,
            TokenType::EndCase => case_depth = case_depth.saturating_sub(1),
            TokenType::Range => {
                let in_case = case_depth > 0;
                let allowed = if in_case {
                    options.allow_bit_string_case_labels
                } else {
                    options.allow_radix_subrange_bounds
                };
                if allowed {
                    continue;
                }
                let before = i.checked_sub(1).map(|j| significant[j]);
                let after = significant.get(i + 1).copied();
                for bound in [before, after].into_iter().flatten() {
                    if is_radix(&bound.token_type) {
                        let problem = if in_case {
                            Problem::BitStringCaseLabelNotAllowed
                        } else {
                            Problem::RadixSubrangeBoundNotAllowed
                        };
                        errors.push(Diagnostic::problem(
                            problem,
                            Label::span(bound.span.clone(), "Radix subrange bound"),
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod test {
    use dsl::core::SourceSpan;

    use crate::{
        options::CompilerOptions,
        rule_token_radix_subrange_bound::apply,
        token::{Token, TokenType},
    };

    fn mk_token(token_type: TokenType, text: &str) -> Token {
        Token {
            token_type,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: text.to_string(),
        }
    }

    /// `16#0 (* c *) .. 15`: a bound separated from `..` by trivia.
    fn hex_start_tokens() -> Vec<Token> {
        vec![
            mk_token(TokenType::HexDigits, "16#0"),
            mk_token(TokenType::Whitespace, " "),
            mk_token(TokenType::Comment, "(* c *)"),
            mk_token(TokenType::Range, ".."),
            mk_token(TokenType::Digits, "15"),
        ]
    }

    fn in_case(mut tokens: Vec<Token>) -> Vec<Token> {
        tokens.insert(0, mk_token(TokenType::Case, "CASE"));
        tokens.push(mk_token(TokenType::EndCase, "END_CASE"));
        tokens
    }

    fn codes(result: Result<(), Vec<dsl::diagnostic::Diagnostic>>) -> Vec<String> {
        result
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|d| d.code)
            .collect()
    }

    #[test]
    fn apply_when_declaration_bound_after_trivia_and_flag_off_then_p4073() {
        let result = apply(&hex_start_tokens(), &CompilerOptions::default());
        assert_eq!(codes(result), vec!["P4073"]);
    }

    #[test]
    fn apply_when_case_bound_and_flag_off_then_p4041() {
        let result = apply(&in_case(hex_start_tokens()), &CompilerOptions::default());
        assert_eq!(codes(result), vec!["P4041"]);
    }

    #[test]
    fn apply_when_bound_after_end_case_then_declaration_flag_applies() {
        let mut tokens = in_case(vec![]);
        tokens.extend(hex_start_tokens());
        let options = CompilerOptions {
            allow_bit_string_case_labels: true,
            ..CompilerOptions::default()
        };
        assert_eq!(codes(apply(&tokens, &options)), vec!["P4073"]);
    }

    #[test]
    fn apply_when_both_bounds_radix_then_error_per_bound() {
        let tokens = vec![
            mk_token(TokenType::BinDigits, "2#0"),
            mk_token(TokenType::Range, ".."),
            mk_token(TokenType::OctDigits, "8#17"),
        ];
        let result = apply(&tokens, &CompilerOptions::default());
        assert_eq!(codes(result), vec!["P4073", "P4073"]);
    }

    fn decimal_tokens() -> Vec<Token> {
        vec![
            mk_token(TokenType::Digits, "0"),
            mk_token(TokenType::Range, ".."),
            mk_token(TokenType::Digits, "15"),
        ]
    }

    #[test]
    fn apply_when_decimal_bounds_then_ok() {
        assert!(apply(&in_case(decimal_tokens()), &CompilerOptions::default()).is_ok());
        assert!(apply(&decimal_tokens(), &CompilerOptions::default()).is_ok());
    }

    #[test]
    fn apply_when_radix_literal_not_next_to_range_then_ok() {
        let tokens = in_case(vec![
            mk_token(TokenType::HexDigits, "16#D012"),
            mk_token(TokenType::Colon, ":"),
        ]);
        assert!(apply(&tokens, &CompilerOptions::default()).is_ok());
    }
}
