//! Validation rule: reject the `STRING(n)` / `WSTRING(n)` parenthesis length
//! delimiter unless the `allow_paren_string_length` flag is set.
//!
//! IEC 61131-3 (Annex B) declares a string length only with square brackets
//! (`STRING [ n ]`). The parenthesis form is a CODESYS/TwinCAT vendor
//! extension, accepted under `--allow-paren-string-length` (implied by the
//! `rusty`, `codesys`, and `twincat` dialects).
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
    use dsl::core::SourceSpan;

    use crate::{
        options::CompilerOptions,
        rule_token_no_paren_string_length::apply,
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

    fn paren_length_tokens(keyword: TokenType) -> Vec<Token> {
        vec![
            mk_token(keyword, "STRING"),
            mk_token(TokenType::LeftParen, "("),
            mk_token(TokenType::Digits, "255"),
            mk_token(TokenType::RightParen, ")"),
        ]
    }

    #[test]
    fn apply_when_string_paren_length_and_flag_off_then_error() {
        let tokens = paren_length_tokens(TokenType::String);
        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_paren_string_length: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_wstring_paren_length_and_flag_off_then_error() {
        let tokens = paren_length_tokens(TokenType::WString);
        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_paren_string_length: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_string_paren_length_and_flag_on_then_ok() {
        let tokens = paren_length_tokens(TokenType::String);
        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_paren_string_length: true,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_string_paren_length_with_whitespace_and_flag_off_then_error() {
        // Whitespace between the keyword and `(` must not hide the delimiter.
        let tokens = vec![
            mk_token(TokenType::String, "STRING"),
            mk_token(TokenType::Whitespace, " "),
            mk_token(TokenType::LeftParen, "("),
            mk_token(TokenType::Digits, "255"),
            mk_token(TokenType::RightParen, ")"),
        ];
        let result = apply(&tokens, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_string_paren_length_with_newline_and_flag_off_then_error() {
        let tokens = vec![
            mk_token(TokenType::String, "STRING"),
            mk_token(TokenType::Newline, "\n"),
            mk_token(TokenType::LeftParen, "("),
            mk_token(TokenType::Digits, "255"),
            mk_token(TokenType::RightParen, ")"),
        ];
        let result = apply(&tokens, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_string_paren_length_with_comment_and_flag_off_then_error() {
        let tokens = vec![
            mk_token(TokenType::String, "STRING"),
            mk_token(TokenType::Comment, "(* n *)"),
            mk_token(TokenType::LeftParen, "("),
            mk_token(TokenType::Digits, "255"),
            mk_token(TokenType::RightParen, ")"),
        ];
        let result = apply(&tokens, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_string_paren_length_with_pragma_and_flag_off_then_error() {
        // The grammar's `_` also skips collapsed Pragma tokens, so the gate
        // must too -- otherwise a pragma between the keyword and `(` would let
        // the paren form through unflagged.
        let tokens = vec![
            mk_token(TokenType::String, "STRING"),
            mk_token(TokenType::Pragma, "{attribute 'x'}"),
            mk_token(TokenType::LeftParen, "("),
            mk_token(TokenType::Digits, "255"),
            mk_token(TokenType::RightParen, ")"),
        ];
        let result = apply(&tokens, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_string_bracket_length_and_flag_off_then_ok() {
        // The standard bracket form is always allowed.
        let tokens = vec![
            mk_token(TokenType::String, "STRING"),
            mk_token(TokenType::LeftBracket, "["),
            mk_token(TokenType::Digits, "255"),
            mk_token(TokenType::RightBracket, "]"),
        ];
        let result = apply(&tokens, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_no_string_keyword_then_ok() {
        let tokens = vec![
            mk_token(TokenType::Identifier, "x"),
            mk_token(TokenType::LeftParen, "("),
        ];
        let result = apply(&tokens, &CompilerOptions::default());
        assert!(result.is_ok());
    }
}
