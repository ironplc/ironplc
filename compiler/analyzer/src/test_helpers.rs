use crate::stages::resolve_types;
use ironplc_dsl::common::*;
use ironplc_dsl::core::FileId;

#[cfg(test)]
pub fn parse_and_resolve_types(program: &str) -> Library {
    use ironplc_parser::parse_program;

    let library = parse_program(program, &FileId::default()).unwrap();
    resolve_types(&vec![&library]).unwrap()
}
