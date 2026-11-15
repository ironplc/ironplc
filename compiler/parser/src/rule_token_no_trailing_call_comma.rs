//! Validation rule: reject a comma before the closing parenthesis of a call's
//! argument list unless the `allow_trailing_call_comma` flag is set.
//!
//! IEC 61131-3 (Annex B) separates arguments with commas and has none after
//! the last one. TwinCAT accepts a trailing comma. It is an extension accepted
//! under `--allow-trailing-call-comma`; which dialects enable it is defined by
//! the dialect mapping in `options.rs`, not restated here.
//!
//! The grammar accepts the form unconditionally (see `param_assignments()`),
//! so this token-stream check enforces the flag. A `,` followed (ignoring
//! trivia) by `)` is unambiguously a trailing comma: no standard construct
//! places a comma directly before a closing parenthesis.
//!
//! The set of tokens skipped here must match the grammar's whitespace rule
//! `_ = (whitespace() / comment() / pragma())*` exactly, or the gate would
//! under-enforce (see `rule_token_no_paren_string_length` for the same
//! reasoning).

use dsl::diagnostic::{Diagnostic, Label};

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

pub fn apply(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_trailing_call_comma {
        return Ok(());
    }

    let mut errors: Vec<Diagnostic> = vec![];
    for (i, tok) in tokens.iter().enumerate() {
        if tok.token_type != TokenType::Comma {
            continue;
        }
        if let Some(next) = tokens[i + 1..].iter().find(|t| !is_trivia(&t.token_type)) {
            if next.token_type == TokenType::RightParen {
                errors.push(Diagnostic::problem(
                    ironplc_problems::Problem::TrailingCallCommaNotAllowed,
                    Label::span(tok.span.clone(), "trailing comma"),
                ));
            }
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

    fn trailing_comma_tokens(between: Option<Token>) -> Vec<Token> {
        let mut tokens = vec![
            token(TokenType::Identifier, "a"),
            token(TokenType::Assignment, ":="),
            token(TokenType::Identifier, "v"),
            token(TokenType::Comma, ","),
        ];
        tokens.extend(between);
        tokens.push(token(TokenType::RightParen, ")"));
        tokens
    }

    token_rule_err!(
        apply_when_trailing_comma_and_flag_off_then_error,
        trailing_comma_tokens(None),
        [Problem::TrailingCallCommaNotAllowed]
    );

    token_rule_ok!(
        apply_when_trailing_comma_and_flag_on_then_ok,
        trailing_comma_tokens(None),
        CompilerOptions {
            allow_trailing_call_comma: true,
            ..CompilerOptions::default()
        }
    );

    // Trivia between the comma and `)` must not hide the form.
    token_rule_err!(
        apply_when_trailing_comma_with_whitespace_and_flag_off_then_error,
        trailing_comma_tokens(Some(token(TokenType::Whitespace, " "))),
        [Problem::TrailingCallCommaNotAllowed]
    );

    token_rule_err!(
        apply_when_trailing_comma_with_newline_and_flag_off_then_error,
        trailing_comma_tokens(Some(token(TokenType::Newline, "\n"))),
        [Problem::TrailingCallCommaNotAllowed]
    );

    token_rule_err!(
        apply_when_trailing_comma_with_comment_and_flag_off_then_error,
        trailing_comma_tokens(Some(token(TokenType::Comment, "(* c *)"))),
        [Problem::TrailingCallCommaNotAllowed]
    );

    token_rule_err!(
        apply_when_trailing_comma_with_pragma_and_flag_off_then_error,
        trailing_comma_tokens(Some(token(TokenType::Pragma, "{attribute 'x'}"))),
        [Problem::TrailingCallCommaNotAllowed]
    );

    token_rule_ok!(
        apply_when_comma_separates_arguments_and_flag_off_then_ok,
        vec![
            token(TokenType::Identifier, "a"),
            token(TokenType::Comma, ","),
            token(TokenType::Identifier, "b"),
            token(TokenType::RightParen, ")"),
        ]
    );
}
