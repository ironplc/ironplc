use crate::stages::resolve_types;
use ironplc_dsl::common::*;
use ironplc_dsl::core::FileId;

#[cfg(test)]
pub fn parse_only(program: &str) -> Library {
    use ironplc_parser::{options::ParseOptions, parse_program};

    parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap()
}

#[cfg(test)]
pub fn parse_and_resolve_types(program: &str) -> Library {
    use ironplc_parser::{options::ParseOptions, parse_program};

    let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
    resolve_types(&[&library]).unwrap()
}
