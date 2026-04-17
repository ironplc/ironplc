//! Validation rule: reject partial-access syntax (`.%Xn`, `.%Bn`, `.%Wn`,
//! `.%Dn`, `.%Ln`) unless the `allow_partial_access_syntax` flag is set.
//! IEC 61131-3:2013 standardizes this form; IronPLC accepts it under
//! `--allow-partial-access-syntax` (implied by the `rusty` and
//! `iec61131-3-ed3` dialects).

use dsl::diagnostic::{Diagnostic, Label};

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

fn is_partial_access_token(t: &TokenType) -> bool {
    matches!(
        t,
        TokenType::PartialAccessBit
            | TokenType::PartialAccessByte
            | TokenType::PartialAccessWord
            | TokenType::PartialAccessDWord
            | TokenType::PartialAccessLWord
    )
}

pub fn apply(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_partial_access_syntax {
        return Ok(());
    }

    let errors: Vec<Diagnostic> = tokens
        .iter()
        .filter(|t| is_partial_access_token(&t.token_type))
        .map(|t| {
            Diagnostic::problem(
                ironplc_problems::Problem::PartialAccessSyntaxDisabled,
                Label::span(t.span.clone(), "partial-access selector"),
            )
        })
        .collect();

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
        rule_token_no_partial_access_syntax::apply,
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

    #[test]
    fn apply_when_partial_access_bit_and_flag_off_then_error() {
        let tokens = vec![mk_token(TokenType::PartialAccessBit, "%X0")];
        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_partial_access_syntax: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_partial_access_bit_and_flag_on_then_ok() {
        let tokens = vec![mk_token(TokenType::PartialAccessBit, "%X0")];
        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_partial_access_syntax: true,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_partial_access_byte_and_flag_off_then_error() {
        let tokens = vec![mk_token(TokenType::PartialAccessByte, "%B0")];
        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_partial_access_syntax: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_partial_access_byte_and_flag_on_then_ok() {
        let tokens = vec![mk_token(TokenType::PartialAccessByte, "%B0")];
        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_partial_access_syntax: true,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_no_partial_access_bit_token_then_ok() {
        let tokens = vec![mk_token(TokenType::Identifier, "x")];
        let result = apply(&tokens, &CompilerOptions::default());
        assert!(result.is_ok());
    }
}
