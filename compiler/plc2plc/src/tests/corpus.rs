//! Round-trip rendering of the shared sample corpus.
//!
//! Each case parses a `.st` source, renders it back to text, and compares
//! against the committed `*_rendered.st` golden output. The cases share one
//! parametrised body so a new corpus entry is a single `#[case]` line.

use super::common::*;
use rstest::rstest;

#[rstest]
#[case::arrays("array.st", "array_rendered.st")]
#[case::wstring_operations("wstring_ops.st", "wstring_ops_rendered.st")]
#[case::array_in_function_var("array_in_function_var.st", "array_in_function_var_rendered.st")]
#[case::conditional("conditional.st", "conditional_rendered.st")]
#[case::configuration("configuration.st", "configuration_rendered.st")]
#[case::expressions("expressions.st", "expressions_rendered.st")]
#[case::inout_var_decl("inout_var_decl.st", "inout_var_decl_rendered.st")]
#[case::input_var_decl("input_var_decl.st", "input_var_decl_rendered.st")]
#[case::literal("literal.st", "literal_rendered.st")]
#[case::nested("nested.st", "nested_rendered.st")]
#[case::program("program.st", "program_rendered.st")]
#[case::sfc("sfc.st", "sfc_rendered.st")]
#[case::strings("strings.st", "strings_rendered.st")]
#[case::textual("textual.st", "textual_rendered.st")]
#[case::type_decl("type_decl.st", "type_decl_rendered.st")]
#[case::var_decl("var_decl.st", "var_decl_rendered.st")]
#[case::sized_string_contexts("sized_string_contexts.st", "sized_string_contexts_rendered.st")]
fn write_to_string_when_corpus_source_then_round_trips(
    #[case] source: &'static str,
    #[case] rendered: &'static str,
) {
    let actual = parse_and_render_resource(source);
    let expected = read_resource(rendered);
    assert_eq!(actual, expected);
}
