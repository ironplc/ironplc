use dsl::diagnostic::{Diagnostic, Label};

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

pub fn apply(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_c_style_comments {
        return Ok(());
    }

    let mut errors = Vec::new();

    for tok in tokens {
        if tok.token_type == TokenType::Comment
            && (tok.text.starts_with("//") || tok.text.starts_with("/*"))
        {
            errors.push(Diagnostic::problem(
                ironplc_problems::Problem::CStyleComment,
                Label::span(tok.span.clone(), "Comment"),
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
        options::CompilerOptions,
        rule_token_no_c_style_comment::apply,
        token::{Token, TokenType},
    };

    #[test]
    fn apply_when_has_cstyle_comment_and_not_allowed_then_error() {
        let tokens = vec![Token {
            token_type: TokenType::Comment,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: String::from("// comment"),
        }];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_c_style_comments: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err())
    }

    #[test]
    fn apply_when_has_cstyle_comment_and_allowed_then_ok() {
        let tokens = vec![Token {
            token_type: TokenType::Comment,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: String::from("// comment"),
        }];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_c_style_comments: true,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok())
    }

    #[test]
    fn apply_when_has_block_comment_and_not_allowed_then_error() {
        let tokens = vec![Token {
            token_type: TokenType::Comment,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: String::from("/* block comment */"),
        }];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_c_style_comments: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err())
    }

    #[test]
    fn apply_when_has_block_comment_and_allowed_then_ok() {
        let tokens = vec![Token {
            token_type: TokenType::Comment,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: String::from("/* block comment */"),
        }];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_c_style_comments: true,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok())
    }
}
