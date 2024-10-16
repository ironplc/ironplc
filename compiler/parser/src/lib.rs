// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl as dsl;

mod lexer;
mod parser;
mod preprocessor;
mod vars;
mod xform_assign_file_id;

use crate::parser::parse_library;
use dsl::{core::FileId, diagnostic::Diagnostic};
use ironplc_dsl::common::Library;
use lexer::{insert_keyword_statement_terminators, tokenize};
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
    let (tokens, errors) = tokenize(&source, file_id);

    let tokens = insert_keyword_statement_terminators(tokens, file_id);

    (tokens, errors)
}

/// Parse a full IEC 61131 program.
pub fn parse_program(source: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    let mut result = tokenize_program(source, file_id);
    if !result.1.is_empty() {
        return Err(result.1.remove(0));
    }

    let library = parse_library(result.0).map(|elements| Library { elements })?;

    // The parser does not know how to assign the file identifier, so transform the input as
    // a post-processing step.
    xform_assign_file_id::apply(library, file_id)
}
