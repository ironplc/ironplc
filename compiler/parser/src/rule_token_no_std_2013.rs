use dsl::diagnostic::{Diagnostic, Label};

use crate::{
    options::ParseOptions,
    token::{Token, TokenType},
};

pub fn apply(tokens: &[Token], options: &ParseOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_iec_61131_3_2013 {
        return Ok(());
    }

    let mut errors = Vec::new();

    for tok in tokens {
        if tok.token_type == TokenType::Ltime {
            errors.push(Diagnostic::problem(
                ironplc_problems::Problem::Std2013Feature,
                Label::span(
                    tok.span.clone(),
                    "LTIME requires --std=iec-61131-3:2013 flag",
                ),
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
        rule_token_no_std_2013::apply,
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
                allow_iec_61131_3_2013: false,
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
                allow_iec_61131_3_2013: true,
                ..ParseOptions::default()
            },
        );
        assert!(result.is_ok())
    }
}
