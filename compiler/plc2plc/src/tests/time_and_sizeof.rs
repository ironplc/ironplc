//! TIME function declarations and SIZEOF rendering.

use super::common::*;

#[test]
fn write_to_string_when_time_function_decl_then_round_trips() {
    let source = read_shared_resource("time_function_decl.st");
    let options = CompilerOptions {
        allow_time_as_function_name: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library).unwrap();
    let expected = read_resource("time_function_decl_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_sizeof() {
    let source = read_shared_resource("sizeof.st");
    let options = CompilerOptions {
        allow_sizeof: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library).unwrap();
    let expected = read_resource("sizeof_rendered.st");
    assert_eq!(rendered, expected);
}
