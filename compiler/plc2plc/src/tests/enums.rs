//! Enumeration explicit values, base types and qualified values.

use super::common::*;

#[test]
fn write_to_string_when_enum_explicit_values_then_round_trips() {
    let source = "
TYPE
E_ModeLanguage : (Deutsch := 1, English := 2);
END_TYPE
";
    let library_original =
        parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("Deutsch := 1"));
    assert!(rendered.contains("English := 2"));

    let library_rendered =
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert_eq!(library_original, library_rendered);
}

#[test]
fn write_to_string_when_enum_base_type_suffix_then_round_trips() {
    let source = "
TYPE
E_AssertionType : (Type_UNDEFINED := 0, Type_ANY, Type_BOOL) BYTE;
END_TYPE
";
    let library_original =
        parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("BYTE"));

    let library_rendered =
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert_eq!(library_original, library_rendered);
}

#[test]
fn write_to_string_when_qualified_enum_value_then_renders_hash() {
    // Regression for a pre-existing bug found while adding
    // explicit_value rendering: COLOR#RED previously rendered as
    // "COLOR RED" (missing the '#'), because there was no dedicated
    // visit_enumerated_value override -- the default recursive
    // visitor used visit_id's write_ws, inserting a space instead of
    // the qualifier separator.
    let source = "
TYPE
COLOR : (RED, GREEN, BLUE);
END_TYPE
FUNCTION_BLOCK FB_Example
VAR
    x : COLOR := COLOR#RED;
END_VAR
END_FUNCTION_BLOCK
";
    let library_original =
        parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("COLOR#RED"));

    let library_rendered =
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert_eq!(library_original, library_rendered);
}
