// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl as dsl;

mod keyword;
mod mapper;
mod parser;
mod preprocessor;

use crate::parser::parse_library;
use dsl::{
    core::FileId,
    diagnostic::{Diagnostic, Label},
};
use ironplc_dsl::common::Library;
use logos::Logos;
use preprocessor::preprocess;
use token::TokenType;

pub mod lexer;
#[cfg(test)]
mod tests;
pub mod token;

/// Tokenize a IEC 61131 program.
///
/// Returns a list of tokens and a list of diagnostics. This does not return a result
/// because we usually continue with parsing even if there are token errors because
/// that will give the context of what was wrong in the location with the error.
pub fn tokenize_program(source: &str, file_id: &FileId) -> (Vec<TokenType>, Vec<Diagnostic>) {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut lexer = TokenType::lexer(source);

    while let Some(token) = lexer.next() {
        match token {
            Ok(tok) => {
                tokens.push(tok);
            }
            Err(_) => {
                let span = lexer.span();
                println!("{:?}", span);
                diagnostics.push(Diagnostic::problem(
                    ironplc_problems::Problem::UnexpectedToken,
                    Label::offset(file_id.clone(), span, "Range of unexpected token"),
                ))
            }
        }
    }

    (tokens, diagnostics)
}

/// Parse a full IEC 61131 program.
pub fn parse_program(source: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    let source = preprocess(source)?;
    parse_library(&source, file_id).map(|elements| Library { elements })
}
