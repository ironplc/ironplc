//! Validation rule: reject a call argument with no value (`b :=,` for an
//! input, `Error=> ,` for an output) unless the `allow_omitted_call_argument`
//! flag is set.
//!
//! IEC 61131-3 (Annex B) gives every formal argument a value or a variable:
//! `param_assignment` is `variable_name ':=' expression` or
//! `variable_name '=>' variable`. An argument written with no value, which
//! TwinCAT uses for an unconnected parameter, is an extension accepted under
//! `--allow-omitted-call-argument`; which dialects enable it is defined by
//! the dialect mapping in `options.rs`, not restated here.
//!
//! The grammar accepts the form unconditionally (see
//! `param_assignment_or_omitted()`), so this token-stream check enforces the
//! flag. An `:=` or `=>` followed (ignoring trivia) by `,` or `)` is
//! unambiguously an argument with no value: no standard construct has an
//! operand missing after either token.
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
    if options.allow_omitted_call_argument {
        return Ok(());
    }

    let mut errors: Vec<Diagnostic> = vec![];
    for (i, tok) in tokens.iter().enumerate() {
        if !matches!(
            tok.token_type,
            TokenType::Assignment | TokenType::RightArrow
        ) {
            continue;
        }
        if let Some(next) = tokens[i + 1..].iter().find(|t| !is_trivia(&t.token_type)) {
            if matches!(next.token_type, TokenType::Comma | TokenType::RightParen) {
                errors.push(Diagnostic::problem(
                    ironplc_problems::Problem::OmittedCallArgumentNotAllowed,
                    Label::span(tok.span.clone(), "argument with no value"),
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

    fn omitted_tokens(operator: TokenType, between: Option<Token>, next: TokenType) -> Vec<Token> {
        let mut tokens = vec![token(TokenType::Identifier, "b"), token(operator, ":=")];
        tokens.extend(between);
        tokens.extend([token(next, ","), token(TokenType::RightParen, ")")]);
        tokens
    }

    token_rule_err!(
        apply_when_input_has_no_value_and_flag_off_then_error,
        omitted_tokens(TokenType::Assignment, None, TokenType::Comma),
        [Problem::OmittedCallArgumentNotAllowed]
    );

    token_rule_err!(
        apply_when_output_has_no_value_and_flag_off_then_error,
        omitted_tokens(TokenType::RightArrow, None, TokenType::Comma),
        [Problem::OmittedCallArgumentNotAllowed]
    );

    token_rule_err!(
        apply_when_last_argument_has_no_value_and_flag_off_then_error,
        vec![
            token(TokenType::Identifier, "b"),
            token(TokenType::Assignment, ":="),
            token(TokenType::RightParen, ")"),
        ],
        [Problem::OmittedCallArgumentNotAllowed]
    );

    token_rule_ok!(
        apply_when_input_has_no_value_and_flag_on_then_ok,
        omitted_tokens(TokenType::Assignment, None, TokenType::Comma),
        CompilerOptions {
            allow_omitted_call_argument: true,
            ..CompilerOptions::default()
        }
    );

    // Trivia between the operator and the comma must not hide the form.
    token_rule_err!(
        apply_when_omitted_with_whitespace_and_flag_off_then_error,
        omitted_tokens(
            TokenType::Assignment,
            Some(token(TokenType::Whitespace, " ")),
            TokenType::Comma
        ),
        [Problem::OmittedCallArgumentNotAllowed]
    );

    token_rule_err!(
        apply_when_omitted_with_newline_and_flag_off_then_error,
        omitted_tokens(
            TokenType::Assignment,
            Some(token(TokenType::Newline, "\n")),
            TokenType::Comma
        ),
        [Problem::OmittedCallArgumentNotAllowed]
    );

    token_rule_err!(
        apply_when_omitted_with_comment_and_flag_off_then_error,
        omitted_tokens(
            TokenType::Assignment,
            Some(token(TokenType::Comment, "(* c *)")),
            TokenType::Comma
        ),
        [Problem::OmittedCallArgumentNotAllowed]
    );

    token_rule_err!(
        apply_when_omitted_with_pragma_and_flag_off_then_error,
        omitted_tokens(
            TokenType::Assignment,
            Some(token(TokenType::Pragma, "{attribute 'x'}")),
            TokenType::Comma
        ),
        [Problem::OmittedCallArgumentNotAllowed]
    );

    token_rule_ok!(
        apply_when_argument_has_value_and_flag_off_then_ok,
        vec![
            token(TokenType::Identifier, "b"),
            token(TokenType::Assignment, ":="),
            token(TokenType::Identifier, "v"),
            token(TokenType::Comma, ","),
            token(TokenType::RightParen, ")"),
        ]
    );
}
