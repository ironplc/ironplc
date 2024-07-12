use crate::stages::resolve_types;
use ironplc_dsl::common::*;
use ironplc_dsl::core::FileId;

#[cfg(test)]
pub fn parse_and_resolve_types(program: &str) -> Library {
    use ironplc_parser::parse_program;

    use crate::compilation_set::CompilationSet;

    let library = parse_program(program, &FileId::default()).unwrap();
    let compilation_set = CompilationSet::of(library);
    resolve_types(&compilation_set).unwrap()
}
