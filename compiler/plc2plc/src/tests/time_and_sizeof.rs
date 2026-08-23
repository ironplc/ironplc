//! TIME function declarations and SIZEOF rendering.

use super::common::*;

#[test]
fn write_to_string_when_time_function_decl_then_round_trips() {
    let options = CompilerOptions {
        allow_time_as_function_name: true,
        ..CompilerOptions::default()
    };
    assert_resource_renders_to(
        "time_function_decl.st",
        "time_function_decl_rendered.st",
        &options,
    );
}

#[test]
fn write_to_string_sizeof() {
    let options = CompilerOptions {
        allow_sizeof: true,
        ..CompilerOptions::default()
    };
    assert_resource_renders_to("sizeof.st", "sizeof_rendered.st", &options);
}
