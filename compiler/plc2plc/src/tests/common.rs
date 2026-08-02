pub(crate) use std::fs;
pub(crate) use std::path::PathBuf;

pub(crate) use dsl::core::FileId;

pub(crate) use ironplc_parser::options::CompilerOptions;
pub(crate) use ironplc_parser::parse_program;
pub(crate) use ironplc_test::read_shared_resource;

pub(crate) use crate::write_to_string;

pub(crate) fn read_resource(name: &'static str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources/test");
    path.push(name);

    fs::read_to_string(path.clone()).unwrap_or_else(|_| panic!("Unable to read file {path:?}"))
}

pub(crate) fn parse_and_render_resource(name: &'static str) -> String {
    let source = read_shared_resource(name);
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    write_to_string(&library).unwrap()
}

pub(crate) fn parse_and_render_resource_with_partial_access(name: &'static str) -> String {
    let source = read_shared_resource(name);
    let options = CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    write_to_string(&library).unwrap()
}

pub(crate) fn parse_and_render_resource_edition3(name: &'static str) -> String {
    let source = read_shared_resource(name);
    let options = CompilerOptions {
        allow_iec_61131_3_2013: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    write_to_string(&library).unwrap()
}

pub(crate) fn parse_and_render_edition3(source: &str) -> String {
    let options = CompilerOptions {
        allow_iec_61131_3_2013: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    write_to_string(&library).unwrap()
}

pub(crate) fn parse_and_render_resource_empty_var_blocks(name: &'static str) -> String {
    let source = read_shared_resource(name);
    let options = CompilerOptions {
        allow_empty_var_blocks: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    write_to_string(&library).unwrap()
}
