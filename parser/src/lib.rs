
extern crate ironplc_dsl as dsl;

mod parser;
mod mapper;

use crate::parser::parse_library;

/// Parse a full IEC 61131 program.
pub fn parse_program(source: &str) -> Result<(), String> {
    parse_library(source)
}