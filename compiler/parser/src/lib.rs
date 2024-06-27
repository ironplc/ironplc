// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl as dsl;

mod lexer;
mod mapper;
mod parser;
mod preprocessor;

use crate::parser::parse_library;
use dsl::{core::FileId, diagnostic::Diagnostic};
use ironplc_dsl::common::Library;
use lexer::tokenize;
use preprocessor::preprocess;
use token::Token;

#[cfg(test)]
mod tests;
pub mod token;

/// Tokenize a IEC 61131 program.
///
/// Returns a list of tokens and a list of diagnostics. This does not return a result
/// because we usually continue with parsing even if there are token errors because
/// that will give the context of what was wrong in the location with the error.
pub fn tokenize_program(source: &str, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>) {
    let source = preprocess(source);
    tokenize(&source, file_id)
}

/// Parse a full IEC 61131 program.
pub fn parse_program(source: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    let mut result = tokenize_program(&source, file_id);
    if !result.1.is_empty() {
        return Err(result.1.remove(0));
    }
    parse_library(result.0).map(|elements| Library { elements })
}
