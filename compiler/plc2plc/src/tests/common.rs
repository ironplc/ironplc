pub(crate) use std::fs;
pub(crate) use std::path::PathBuf;

pub(crate) use dsl::core::FileId;

pub(crate) use ironplc_parser::options::{CompilerOptions, Dialect};
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
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    write_to_string(&library).unwrap()
}

pub(crate) fn parse_and_render_edition3(source: &str) -> String {
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
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

/// Asserts that `source` survives a parse -> render -> re-parse round trip
/// with the AST unchanged.
///
/// Rendering alone does not prove the output is valid input: a stray space
/// can produce text the parser rejects (see the `^` and `[` spacing bugs).
/// Re-parsing here is what catches that.
pub(crate) fn assert_round_trips(source: &str, options: &CompilerOptions) {
    let library_original = parse_program(source, &FileId::default(), options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    let library_rendered = parse_program(&rendered, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Rendered output did not re-parse: {e:?}\n{rendered}"));
    assert_eq!(
        library_original, library_rendered,
        "Round trip changed the AST. Rendered:\n{rendered}"
    );
}
