extern crate ironplc_dsl as dsl;

mod mapper;
mod parser;

use crate::parser::parse_library;
use ironplc_dsl::dsl::Library;

/// Parse a full IEC 61131 program.
pub fn parse_program(source: &str) -> Result<Library, String> {
    parse_library(source).map(|elems| Library { elems: elems })
}
