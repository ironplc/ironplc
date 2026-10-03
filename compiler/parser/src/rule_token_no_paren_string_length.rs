//! Validation rule: reject the `STRING(n)` / `WSTRING(n)` parenthesis length
//! delimiter unless the `allow_paren_string_length` flag is set.
//!
//! IEC 61131-3 (Annex B) declares a string length only with square brackets
//! (`STRING [ n ]`). The parenthesis form is an extension, accepted
//! under `--allow-paren-string-length`; which dialects enable it is defined
//! by the dialect mapping in `options.rs`, not restated here.
//!
//! The grammar accepts the parenthesis form unconditionally (see
//! `string_length_spec()` and `string_type_declaration__parenthesis()`), so
//! this token-stream check is what enforces the flag. A `STRING`/`WSTRING`
//! keyword followed (ignoring trivia) by `(` is unambiguously a length
//! delimiter -- neither keyword is callable and typed string literals use
//! `STRING#`, so no standard construct places `(` directly after the keyword.
//!
//! The set of tokens skipped here must match the grammar's whitespace rule
//! `_ = (whitespace() / comment() / pragma())*` exactly, or the gate would
//! under-enforce: e.g. `STRING {attribute 'x'} (255)` parses (the grammar's
//! `_` skips the collapsed `Pragma` token) and must be rejected the same way
//! a plain `STRING (255)` is. `Pragma` tokens only exist when `allow_pragmas`
//! is set (`xform_collapse_pragmas` runs before this check); otherwise the
//! braces surface as their own tokens and never sit between the keyword and
//! `(`.

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
    if options.allow_paren_string_length {
        return Ok(());
    }

    let mut errors: Vec<Diagnostic> = vec![];
    for (i, tok) in tokens.iter().enumerate() {
        if !matches!(tok.token_type, TokenType::String | TokenType::WString) {
            continue;
        }
        // The next significant token after the keyword decides whether this is
        // the parenthesis length form.
        if let Some(next) = tokens[i + 1..].iter().find(|t| !is_trivia(&t.token_type)) {
            if next.token_type == TokenType::LeftParen {
                errors.push(Diagnostic::problem(
                    ironplc_problems::Problem::ParenStringLengthNotAllowed,
                    Label::span(next.span.clone(), "parenthesis length delimiter"),
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

    fn paren_length_tokens(keyword: TokenType) -> Vec<Token> {
        vec![
            token(keyword, "STRING"),
            token(TokenType::LeftParen, "("),
            token(TokenType::Digits, "255"),
            token(TokenType::RightParen, ")"),
        ]
    }

    token_rule_err!(
        apply_when_string_paren_length_and_flag_off_then_error,
        paren_length_tokens(TokenType::String),
        [Problem::ParenStringLengthNotAllowed],
        CompilerOptions {
            allow_paren_string_length: false,
            ..CompilerOptions::default()
        }
    );

    token_rule_err!(
        apply_when_wstring_paren_length_and_flag_off_then_error,
        paren_length_tokens(TokenType::WString),
        [Problem::ParenStringLengthNotAllowed],
        CompilerOptions {
            allow_paren_string_length: false,
            ..CompilerOptions::default()
        }
    );

    token_rule_ok!(
        apply_when_string_paren_length_and_flag_on_then_ok,
        paren_length_tokens(TokenType::String),
        CompilerOptions {
            allow_paren_string_length: true,
            ..CompilerOptions::default()
        }
    );

    // Whitespace between the keyword and `(` must not hide the delimiter.
    token_rule_err!(
        apply_when_string_paren_length_with_whitespace_and_flag_off_then_error,
        vec![
            token(TokenType::String, "STRING"),
            token(TokenType::Whitespace, " "),
            token(TokenType::LeftParen, "("),
            token(TokenType::Digits, "255"),
            token(TokenType::RightParen, ")"),
        ],
        [Problem::ParenStringLengthNotAllowed]
    );

    token_rule_err!(
        apply_when_string_paren_length_with_newline_and_flag_off_then_error,
        vec![
            token(TokenType::String, "STRING"),
            token(TokenType::Newline, "\n"),
            token(TokenType::LeftParen, "("),
            token(TokenType::Digits, "255"),
            token(TokenType::RightParen, ")"),
        ],
        [Problem::ParenStringLengthNotAllowed]
    );

    token_rule_err!(
        apply_when_string_paren_length_with_comment_and_flag_off_then_error,
        vec![
            token(TokenType::String, "STRING"),
            token(TokenType::Comment, "(* n *)"),
            token(TokenType::LeftParen, "("),
            token(TokenType::Digits, "255"),
            token(TokenType::RightParen, ")"),
        ],
        [Problem::ParenStringLengthNotAllowed]
    );

    // The grammar's `_` also skips collapsed Pragma tokens, so the gate
    // must too -- otherwise a pragma between the keyword and `(` would let
    // the paren form through unflagged.
    token_rule_err!(
        apply_when_string_paren_length_with_pragma_and_flag_off_then_error,
        vec![
            token(TokenType::String, "STRING"),
            token(TokenType::Pragma, "{attribute 'x'}"),
            token(TokenType::LeftParen, "("),
            token(TokenType::Digits, "255"),
            token(TokenType::RightParen, ")"),
        ],
        [Problem::ParenStringLengthNotAllowed]
    );

    // The standard bracket form is always allowed.
    token_rule_ok!(
        apply_when_string_bracket_length_and_flag_off_then_ok,
        vec![
            token(TokenType::String, "STRING"),
            token(TokenType::LeftBracket, "["),
            token(TokenType::Digits, "255"),
            token(TokenType::RightBracket, "]"),
        ]
    );

    token_rule_ok!(
        apply_when_no_string_keyword_then_ok,
        vec![
            token(TokenType::Identifier, "x"),
            token(TokenType::LeftParen, "("),
        ]
    );
}
