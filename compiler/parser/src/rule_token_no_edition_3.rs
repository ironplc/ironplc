use dsl::diagnostic::{Diagnostic, Label};

use crate::{
    options::ParseOptions,
    token::{Token, TokenType},
};

pub fn apply(tokens: &[Token], options: &ParseOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_edition_3 {
        return Ok(());
    }

    let mut errors = Vec::new();

    for tok in tokens {
        if tok.token_type == TokenType::Ltime {
            errors.push(Diagnostic::problem(
                ironplc_problems::Problem::Edition3Feature,
                Label::span(tok.span.clone(), "LTIME requires --edition-3 flag"),
            ));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use dsl::core::SourceSpan;

    use crate::{
        options::ParseOptions,
        rule_token_no_edition_3::apply,
        token::{Token, TokenType},
    };

    #[test]
    fn apply_when_has_ltime_and_not_allowed_then_error() {
        let tokens = vec![Token {
            token_type: TokenType::Ltime,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: String::from("LTIME"),
        }];

        let result = apply(
            &tokens,
            &ParseOptions {
                allow_edition_3: false,
                ..ParseOptions::default()
            },
        );
        assert!(result.is_err())
    }

    #[test]
    fn apply_when_has_ltime_and_allowed_then_ok() {
        let tokens = vec![Token {
            token_type: TokenType::Ltime,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: String::from("LTIME"),
        }];

        let result = apply(
            &tokens,
            &ParseOptions {
                allow_edition_3: true,
                ..ParseOptions::default()
            },
        );
        assert!(result.is_ok())
    }
}
