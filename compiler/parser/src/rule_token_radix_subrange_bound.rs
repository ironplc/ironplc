//! Validation rule: reject a subrange bound written in hex, binary or octal
//! (`16#01..16#0F`) unless the flag for its context is set.
//!
//! IEC 61131-3 (Annex B) defines `subrange ::= signed_integer '..'
//! signed_integer`, where `signed_integer` is a decimal digit sequence, so a
//! radix bound is an extension. The grammar accepts it unconditionally (see
//! `subrange_bound()`) and holds it as the same literal a decimal bound is,
//! so this token-stream check is what enforces the flags:
//!
//! - In a `CASE` label, `--allow-radix-case-labels` (P4041), the flag
//!   that already gates a radix label such as `16#D012:`.
//! - In a subrange type or an array dimension,
//!   `--allow-radix-subrange-bounds` (P4096).
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
    if options.allow_radix_case_labels && options.allow_radix_subrange_bounds {
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
                    options.allow_radix_case_labels
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
                            Problem::RadixCaseLabelNotAllowed
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
    use crate::test_rule_helpers::token;
    use ironplc_problems::Problem;

    use crate::{
        options::CompilerOptions,
        token::{Token, TokenType},
    };

    /// `16#0 (* c *) .. 15`: a bound separated from `..` by trivia.
    fn hex_start_tokens() -> Vec<Token> {
        vec![
            token(TokenType::HexDigits, "16#0"),
            token(TokenType::Whitespace, " "),
            token(TokenType::Comment, "(* c *)"),
            token(TokenType::Range, ".."),
            token(TokenType::Digits, "15"),
        ]
    }

    fn in_case(mut tokens: Vec<Token>) -> Vec<Token> {
        tokens.insert(0, token(TokenType::Case, "CASE"));
        tokens.push(token(TokenType::EndCase, "END_CASE"));
        tokens
    }

    fn decimal_tokens() -> Vec<Token> {
        vec![
            token(TokenType::Digits, "0"),
            token(TokenType::Range, ".."),
            token(TokenType::Digits, "15"),
        ]
    }

    fn after_end_case(tokens: Vec<Token>) -> Vec<Token> {
        let mut all = in_case(vec![]);
        all.extend(tokens);
        all
    }

    token_rule_err!(
        apply_when_declaration_bound_after_trivia_and_flag_off_then_error,
        hex_start_tokens(),
        [Problem::RadixSubrangeBoundNotAllowed]
    );

    token_rule_ok!(
        apply_when_declaration_bound_and_flag_on_then_ok,
        hex_start_tokens(),
        CompilerOptions {
            allow_radix_subrange_bounds: true,
            ..CompilerOptions::default()
        }
    );

    token_rule_err!(
        apply_when_case_bound_and_flag_off_then_error,
        in_case(hex_start_tokens()),
        [Problem::RadixCaseLabelNotAllowed]
    );

    token_rule_ok!(
        apply_when_case_bound_and_flag_on_then_ok,
        in_case(hex_start_tokens()),
        CompilerOptions {
            allow_radix_case_labels: true,
            ..CompilerOptions::default()
        }
    );

    token_rule_err!(
        apply_when_bound_after_end_case_then_declaration_flag_applies,
        after_end_case(hex_start_tokens()),
        [Problem::RadixSubrangeBoundNotAllowed],
        CompilerOptions {
            allow_radix_case_labels: true,
            ..CompilerOptions::default()
        }
    );

    token_rule_err!(
        apply_when_both_bounds_radix_then_error_per_bound,
        vec![
            token(TokenType::BinDigits, "2#0"),
            token(TokenType::Range, ".."),
            token(TokenType::OctDigits, "8#17"),
        ],
        [Problem::RadixSubrangeBoundNotAllowed; 2]
    );

    token_rule_ok!(
        apply_when_case_decimal_bounds_then_ok,
        in_case(decimal_tokens())
    );

    token_rule_ok!(
        apply_when_declaration_decimal_bounds_then_ok,
        decimal_tokens()
    );

    token_rule_ok!(
        apply_when_radix_literal_not_next_to_range_then_ok,
        in_case(vec![
            token(TokenType::HexDigits, "16#D012"),
            token(TokenType::Colon, ":"),
        ])
    );
}
