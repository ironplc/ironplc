// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl as dsl;

mod lexer;
pub mod options;
mod parser;
mod preprocessor;
mod rule_token_no_c_style_comment;
mod vars;
mod xform_assign_file_id;
mod xform_tokens;

use crate::parser::parse_library;
use dsl::{core::FileId, diagnostic::Diagnostic};
use ironplc_dsl::common::Library;
use lexer::tokenize;
use options::ParseOptions;
use preprocessor::preprocess;
use token::Token;
use xform_tokens::insert_keyword_statement_terminators;

#[cfg(test)]
mod tests;
pub mod token;

/// Tokenize a IEC 61131 program.
///
/// Returns a list of tokens and a list of diagnostics. This does not return a result
/// because we usually continue with parsing even if there are token errors because
/// that will give the context of what was wrong in the location with the error.
pub fn tokenize_program(
    source: &str,
    file_id: &FileId,
    options: &ParseOptions,
) -> (Vec<Token>, Vec<Diagnostic>) {
    let source = preprocess(source);
    let (tokens, mut errors) = tokenize(&source, file_id);

    let tokens = insert_keyword_statement_terminators(tokens, file_id);
    let result = check_tokens(&tokens, options);
    match result {
        Ok(_) => {}
        Err(mut diagnostics) => errors.append(&mut diagnostics),
    }

    (tokens, errors)
}

#[allow(clippy::type_complexity)]
fn check_tokens(tokens: &[Token], options: &ParseOptions) -> Result<(), Vec<Diagnostic>> {
    let rules: Vec<fn(&[Token], &ParseOptions) -> Result<(), Vec<Diagnostic>>> =
        vec![rule_token_no_c_style_comment::apply];

    let mut errors = vec![];
    for rule in rules {
        match rule(tokens, options) {
            Ok(_) => {}
            Err(mut diagnostics) => errors.append(&mut diagnostics),
        };
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(())
}

/// Parse a full IEC 61131 program.
pub fn parse_program(
    source: &str,
    file_id: &FileId,
    options: &ParseOptions,
) -> Result<Library, Diagnostic> {
    let mut result = tokenize_program(source, file_id, options);
    if !result.1.is_empty() {
        return Err(result.1.remove(0));
    }

    let library = parse_library(result.0).map(|elements| Library { elements })?;

    // The parser does not know how to assign the file identifier, so transform the input as
    // a post-processing step.
    xform_assign_file_id::apply(library, file_id)
}
