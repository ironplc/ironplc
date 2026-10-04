//! Validation rule: reject partial-access syntax (`.%Xn`, `.%Bn`, `.%Wn`,
//! `.%Dn`, `.%Ln`) unless the `allow_partial_access_syntax` flag is set.
//! IEC 61131-3:2013 standardizes this form; IronPLC accepts it under
//! `--allow-partial-access-syntax`.

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
    use crate::test_rule_helpers::{result_codes, token};
    use ironplc_problems::Problem;
    use spec_test_macro::spec_test;

    use crate::{
        options::CompilerOptions, rule_token_no_partial_access_syntax::apply, token::TokenType,
    };

    token_rule_err!(
        apply_when_partial_access_bit_and_flag_off_then_error,
        vec![token(TokenType::PartialAccessBit, "%X0")],
        [Problem::PartialAccessSyntaxDisabled],
        CompilerOptions {
            allow_partial_access_syntax: false,
            ..CompilerOptions::default()
        }
    );

    token_rule_ok!(
        apply_when_partial_access_bit_and_flag_on_then_ok,
        vec![token(TokenType::PartialAccessBit, "%X0")],
        CompilerOptions {
            allow_partial_access_syntax: true,
            ..CompilerOptions::default()
        }
    );

    /// REQ-PAB-parser-140: a wider selector without the flag is
    /// `PartialAccessSyntaxDisabled`, the same diagnostic as `.%Xn`.
    #[spec_test(REQ_PAB_parser_140)]
    fn apply_when_partial_access_byte_and_flag_off_then_error() {
        let tokens = vec![token(TokenType::PartialAccessByte, "%B0")];
        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_partial_access_syntax: false,
                ..CompilerOptions::default()
            },
        );
        let codes = result_codes(&result);
        assert_eq!(
            codes,
            [ironplc_problems::Problem::PartialAccessSyntaxDisabled.code()]
        );
    }

    token_rule_ok!(
        apply_when_partial_access_byte_and_flag_on_then_ok,
        vec![token(TokenType::PartialAccessByte, "%B0")],
        CompilerOptions {
            allow_partial_access_syntax: true,
            ..CompilerOptions::default()
        }
    );

    token_rule_ok!(
        apply_when_no_partial_access_bit_token_then_ok,
        vec![token(TokenType::Identifier, "x")]
    );
}
