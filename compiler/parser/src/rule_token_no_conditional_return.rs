//! Validation rule: reject the `RETURN(condition)` conditional return unless
//! the `allow_conditional_return` flag is set.
//!
//! IEC 61131-3 (Annex B) defines `RETURN` with no operand. The parenthesized
//! form, which returns only when the `BOOL` condition is TRUE, is an
//! extension accepted under `--allow-conditional-return`; which dialects
//! enable it is defined by the dialect mapping in `options.rs`, not restated
//! here.
//!
//! The grammar accepts the form unconditionally (see
//! `subprogram_control_statement()`), so this token-stream check is what
//! enforces the flag. A `RETURN` keyword followed (ignoring trivia) by `(` is
//! unambiguously the conditional form: in the standard grammar `RETURN` is a
//! complete statement, and no statement starts with `(`.
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
    if options.allow_conditional_return {
        return Ok(());
    }

    let mut errors: Vec<Diagnostic> = vec![];
    for (i, tok) in tokens.iter().enumerate() {
        if tok.token_type != TokenType::Return {
            continue;
        }
        if let Some(next) = tokens[i + 1..].iter().find(|t| !is_trivia(&t.token_type)) {
            if next.token_type == TokenType::LeftParen {
                errors.push(Diagnostic::problem(
                    ironplc_problems::Problem::ConditionalReturnNotAllowed,
                    Label::span(tok.span.clone(), "conditional RETURN"),
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

    fn conditional_return_tokens(between: Option<Token>) -> Vec<Token> {
        let mut tokens = vec![token(TokenType::Return, "RETURN")];
        tokens.extend(between);
        tokens.extend([
            token(TokenType::LeftParen, "("),
            token(TokenType::Identifier, "b"),
            token(TokenType::RightParen, ")"),
            token(TokenType::Semicolon, ";"),
        ]);
        tokens
    }

    token_rule_err!(
        apply_when_conditional_return_and_flag_off_then_error,
        conditional_return_tokens(None),
        [Problem::ConditionalReturnNotAllowed]
    );

    token_rule_ok!(
        apply_when_conditional_return_and_flag_on_then_ok,
        conditional_return_tokens(None),
        CompilerOptions {
            allow_conditional_return: true,
            ..CompilerOptions::default()
        }
    );

    // Trivia between the keyword and `(` must not hide the conditional form.
    token_rule_err!(
        apply_when_conditional_return_with_whitespace_and_flag_off_then_error,
        conditional_return_tokens(Some(token(TokenType::Whitespace, " "))),
        [Problem::ConditionalReturnNotAllowed]
    );

    token_rule_err!(
        apply_when_conditional_return_with_newline_and_flag_off_then_error,
        conditional_return_tokens(Some(token(TokenType::Newline, "\n"))),
        [Problem::ConditionalReturnNotAllowed]
    );

    token_rule_err!(
        apply_when_conditional_return_with_comment_and_flag_off_then_error,
        conditional_return_tokens(Some(token(TokenType::Comment, "(* c *)"))),
        [Problem::ConditionalReturnNotAllowed]
    );

    token_rule_err!(
        apply_when_conditional_return_with_pragma_and_flag_off_then_error,
        conditional_return_tokens(Some(token(TokenType::Pragma, "{attribute 'x'}"))),
        [Problem::ConditionalReturnNotAllowed]
    );

    token_rule_ok!(
        apply_when_plain_return_and_flag_off_then_ok,
        vec![
            token(TokenType::Return, "RETURN"),
            token(TokenType::Semicolon, ";"),
        ]
    );
}
