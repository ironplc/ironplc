extern crate ironplc_dsl as dsl;

mod mapper;
mod parser;

use crate::parser::parse_library;
use ironplc_dsl::dsl::LibraryElement;

/// Parse a full IEC 61131 program.
pub fn parse_program(source: &str) -> Result<Vec<LibraryElement>, String> {
    parse_library(source)
}
