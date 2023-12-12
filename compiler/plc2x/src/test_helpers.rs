use ironplc_dsl::common::*;
use ironplc_dsl::core::FileId;
use crate::stages::resolve_types;
use crate::stages::parse;
use crate::stages::CompilationSet;


use std::fs;
use std::path::PathBuf;

#[cfg(test)]
pub fn read_resource(name: &'static str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources/test");
    path.push(name);

    fs::read_to_string(path).expect("Unable to read file")
}

#[cfg(test)]
pub fn resource_path(name: &'static str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources/test");
    path.push(name);
    path
}

#[cfg(test)]
pub fn new_library(element: LibraryElementKind) -> Library {
    Library {
        elements: vec![element],
    }
}

#[cfg(test)]
pub fn parse_and_resolve_types(program: &str) -> Library {
    let library = parse(program, &FileId::default()).unwrap();
    let compilation_set = CompilationSet::of(library);
    let library = resolve_types(&compilation_set).unwrap();
    library
}
