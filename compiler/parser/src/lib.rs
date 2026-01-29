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

use crate::parser::{parse_library, parse_statements};
use dsl::{core::FileId, diagnostic::Diagnostic};
use ironplc_dsl::common::Library;
use ironplc_dsl::textual::StmtKind;
use lexer::tokenize_with_offset;
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
    tokenize_program_with_offset(source, file_id, options, 0, 0, 0)
}

/// Tokenize a IEC 61131 program with an initial offset.
///
/// This is useful for tokenizing embedded content (like ST body from XML) where
/// the content doesn't start at the beginning of the file.
///
/// - `byte_offset`: The byte position in the original file where this content starts
/// - `line_offset`: The line number (0-based) where this content starts
/// - `col_offset`: The column number (0-based) where this content starts
pub fn tokenize_program_with_offset(
    source: &str,
    file_id: &FileId,
    options: &ParseOptions,
    byte_offset: usize,
    line_offset: usize,
    col_offset: usize,
) -> (Vec<Token>, Vec<Diagnostic>) {
    let source = preprocess(source);
    let (tokens, mut errors) =
        tokenize_with_offset(&source, file_id, byte_offset, line_offset, col_offset);

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

/// Parse ST (Structured Text) body content into statements.
///
/// This is useful for parsing ST body content from PLCopen XML files
/// where only the statements (not the full POU declaration) are provided.
pub fn parse_st_statements(
    source: &str,
    file_id: &FileId,
    options: &ParseOptions,
) -> Result<Vec<StmtKind>, Diagnostic> {
    parse_st_statements_with_offset(source, file_id, options, 0, 0, 0)
}

/// Parse ST (Structured Text) body content into statements with position offset.
///
/// This is useful for parsing ST body content from PLCopen XML files
/// where the content is embedded and doesn't start at the beginning of the file.
///
/// - `byte_offset`: The byte position in the original file where this content starts
/// - `line_offset`: The line number (0-based) where this content starts
/// - `col_offset`: The column number (0-based) where this content starts
pub fn parse_st_statements_with_offset(
    source: &str,
    file_id: &FileId,
    options: &ParseOptions,
    byte_offset: usize,
    line_offset: usize,
    col_offset: usize,
) -> Result<Vec<StmtKind>, Diagnostic> {
    if source.trim().is_empty() {
        return Ok(vec![]);
    }

    let mut result = tokenize_program_with_offset(
        source,
        file_id,
        options,
        byte_offset,
        line_offset,
        col_offset,
    );
    if !result.1.is_empty() {
        return Err(result.1.remove(0));
    }

    parse_statements(result.0)
}
