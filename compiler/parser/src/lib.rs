// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl as dsl;

mod keyword;
mod mapper;
mod parser;
mod preprocessor;

use crate::parser::parse_library;
use dsl::{core::FileId, diagnostic::Diagnostic};
use ironplc_dsl::common::Library;
use logos::Logos;
use preprocessor::preprocess;
use token::TokenType;

pub mod lexer;
#[cfg(test)]
mod tests;
pub mod token;

/// Tokenize a IEC 61131 program.
pub fn tokenize_program(
    source: &str,
    _file_id: &FileId,
) -> Result<Vec<TokenType>, Vec<Diagnostic>> {
    let mut tokens = Vec::new();
    let lexer = TokenType::lexer(source);
    for tok in lexer.flatten() {
        tokens.push(tok)
    }
    Ok(tokens)
}

/// Parse a full IEC 61131 program.
pub fn parse_program(source: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    let source = preprocess(source)?;
    parse_library(&source, file_id).map(|elements| Library { elements })
}
