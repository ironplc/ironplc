// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl as dsl;

mod mapper;
mod parser;
mod preprocessor;

use crate::parser::parse_library;
use dsl::{core::FileId, diagnostic::Diagnostic};
use ironplc_dsl::common::Library;
use preprocessor::preprocess;

#[cfg(test)]
mod tests;

/// Parse a full IEC 61131 program.
pub fn parse_program(source: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    let source = preprocess(source, file_id)?;
    parse_library(&source, file_id).map(|elements| Library { elements })
}
