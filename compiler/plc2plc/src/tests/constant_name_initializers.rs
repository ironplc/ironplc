//! Round-tripping of a `VAR` initializer that names a constant.

use super::common::*;

/// The name renders as written, so a declaration that names a global
/// constant re-parses to the same constant expression. The parser does not
/// consult the dialect for this, so the default options suffice.
#[test]
fn write_to_string_when_initializer_names_constant_then_round_trips() {
    let source = "
VAR_GLOBAL CONSTANT
    C : UDINT := 1;
END_VAR
FUNCTION_BLOCK FB_A
VAR
    x : UDINT := C;
    y : UDINT := C + 1;
END_VAR
END_FUNCTION_BLOCK
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_type_default_names_constant_then_round_trips() {
    let source = "
VAR_GLOBAL CONSTANT
    C : UDINT := 1;
END_VAR
TYPE
    MyInt : UDINT := C;
END_TYPE
";
    assert_round_trips(source, &CompilerOptions::default());
}

/// A string initializer renders its width and length as declared and the
/// name as written, for a variable and for a structure element. The two
/// constants are in blocks of their own because the renderer writes one
/// block per global declaration.
#[test]
fn write_to_string_when_string_initializer_names_constant_then_round_trips() {
    let source = "
VAR_GLOBAL CONSTANT
    CS : STRING := 'abc';
END_VAR
VAR_GLOBAL CONSTANT
    WS : WSTRING := \"abc\";
END_VAR
FUNCTION_BLOCK FB_A
VAR
    s : STRING := CS;
    t : STRING[20] := CS;
    w : WSTRING[8] := WS;
END_VAR
END_FUNCTION_BLOCK
TYPE
    Pt : STRUCT
        name : STRING[10] := CS;
    END_STRUCT;
END_TYPE
";
    assert_round_trips(source, &CompilerOptions::default());
}
