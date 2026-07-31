//! Round-trip rendering of the shared sample corpus.

use super::common::*;

#[test]
fn write_to_string_arrays() {
    let rendered = parse_and_render_resource("array.st");
    let expected = read_resource("array_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_when_wstring_operations_then_round_trips() {
    let rendered = parse_and_render_resource("wstring_ops.st");
    let expected = read_resource("wstring_ops_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_when_array_in_function_var_then_renders() {
    let rendered = parse_and_render_resource("array_in_function_var.st");
    let expected = read_resource("array_in_function_var_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_conditional() {
    let rendered = parse_and_render_resource("conditional.st");
    let expected = read_resource("conditional_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_configuration() {
    let rendered = parse_and_render_resource("configuration.st");
    let expected = read_resource("configuration_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_expressions() {
    let rendered = parse_and_render_resource("expressions.st");
    let expected = read_resource("expressions_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_inout_var_decl() {
    let rendered = parse_and_render_resource("inout_var_decl.st");
    let expected = read_resource("inout_var_decl_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_input_var_decl() {
    let rendered = parse_and_render_resource("input_var_decl.st");
    let expected = read_resource("input_var_decl_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_literal() {
    let rendered = parse_and_render_resource("literal.st");
    let expected = read_resource("literal_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_nested() {
    let rendered = parse_and_render_resource("nested.st");
    let expected = read_resource("nested_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_program() {
    let rendered = parse_and_render_resource("program.st");
    let expected = read_resource("program_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_sfc() {
    let rendered = parse_and_render_resource("sfc.st");
    let expected = read_resource("sfc_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_strings() {
    let rendered = parse_and_render_resource("strings.st");
    let expected = read_resource("strings_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_textual() {
    let rendered = parse_and_render_resource("textual.st");
    let expected = read_resource("textual_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_type_decl() {
    let rendered = parse_and_render_resource("type_decl.st");
    let expected = read_resource("type_decl_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_var_decl() {
    let rendered = parse_and_render_resource("var_decl.st");
    let expected = read_resource("var_decl_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_sized_string_contexts() {
    let rendered = parse_and_render_resource("sized_string_contexts.st");
    let expected = read_resource("sized_string_contexts_rendered.st");
    assert_eq!(rendered, expected);
}
