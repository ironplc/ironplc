//! Shared setup for the token rule tests (`rule_*.rs`).

use dsl::core::SourceSpan;
use dsl::diagnostic::Diagnostic;

use crate::token::{Token, TokenType};

/// A token of `token_type` spelled `text`, at the start of the file.
pub fn token(token_type: TokenType, text: &str) -> Token {
    Token {
        token_type,
        span: SourceSpan::default(),
        line: 1,
        col: 1,
        text: text.to_string(),
    }
}

/// The problem codes a token rule reported, in order (none when it accepted).
pub fn result_codes(result: &Result<(), Vec<Diagnostic>>) -> Vec<&str> {
    match result {
        Ok(()) => vec![],
        Err(diagnostics) => diagnostics.iter().map(|d| d.code.as_str()).collect(),
    }
}
