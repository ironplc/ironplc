//! Enumeration explicit values, base types and qualified values.

use super::common::*;

#[test]
fn write_to_string_when_enum_explicit_values_then_round_trips() {
    let rendered = assert_round_trips(
        "
TYPE
E_ModeLanguage : (Deutsch := 1, English := 2);
END_TYPE
",
        &CompilerOptions::default(),
    );

    assert!(rendered.contains("Deutsch := 1"));
    assert!(rendered.contains("English := 2"));
}

#[test]
fn write_to_string_when_enum_base_type_suffix_then_round_trips() {
    let rendered = assert_round_trips(
        "
TYPE
E_AssertionType : (Type_UNDEFINED := 0, Type_ANY, Type_BOOL) BYTE;
END_TYPE
",
        &CompilerOptions::default(),
    );

    assert!(rendered.contains("BYTE"));
}

#[test]
fn write_to_string_when_qualified_enum_value_then_renders_hash() {
    // Regression for a pre-existing bug found while adding
    // explicit_value rendering: COLOR#RED previously rendered as
    // "COLOR RED" (missing the '#'), because there was no dedicated
    // visit_enumerated_value override -- the default recursive
    // visitor used visit_id's write_ws, inserting a space instead of
    // the qualifier separator.
    let rendered = assert_round_trips(
        "
TYPE
COLOR : (RED, GREEN, BLUE);
END_TYPE
FUNCTION_BLOCK FB_Example
VAR
    x : COLOR := COLOR#RED;
END_VAR
END_FUNCTION_BLOCK
",
        &CompilerOptions::default(),
    );

    assert!(rendered.contains("COLOR#RED"));
}
