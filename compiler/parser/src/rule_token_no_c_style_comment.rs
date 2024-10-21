use dsl::diagnostic::{Diagnostic, Label};

use crate::{
    options::ParseOptions,
    token::{Token, TokenType},
};

pub fn apply(tokens: &[Token], options: &ParseOptions) -> Result<(), Vec<Diagnostic>> {
    if !options.allow_c_style_comments {
        return Ok(());
    }

    let mut errors = Vec::new();

    for tok in tokens {
        if tok.token_type == TokenType::Comment && tok.text.starts_with("//") {
            errors.push(Diagnostic::problem(
                ironplc_problems::Problem::CStyleComment,
                Label::span(tok.span.clone(), "Comment"),
            ));
        }
    }
    Ok(())
}
