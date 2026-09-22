//! Enumeration explicit values, base types and qualified values.

use super::common::*;

#[test]
fn write_to_string_when_enum_explicit_values_then_round_trips() {
    // Explicit member values are gated by `allow_enum_explicit_values`. The
    // grammar recognizes them either way (the rejection is a semantic rule,
    // not a parse failure), but the source is dialect syntax, so the round
    // trip states the dialect it belongs to.
    assert_round_trips(
        "
TYPE
E_ModeLanguage : (Deutsch := 1, English := 2);
END_TYPE
",
        &CompilerOptions {
            allow_enum_explicit_values: true,
            ..CompilerOptions::default()
        },
    );
}

#[test]
fn write_to_string_when_enum_base_type_suffix_then_round_trips() {
    assert_round_trips(
        "
TYPE
E_AssertionType : (Type_UNDEFINED := 0, Type_ANY, Type_BOOL) BYTE;
END_TYPE
",
        &CompilerOptions {
            allow_enum_explicit_values: true,
            ..CompilerOptions::default()
        },
    );
}

#[test]
fn write_to_string_when_qualified_enum_value_then_renders_hash() {
    // A qualified value needs its `#` separator; the default recursive
    // visit would write a space.
    assert_round_trips(
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
}
