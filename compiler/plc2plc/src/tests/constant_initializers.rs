//! Constant-expression VAR initializer round-tripping.

use super::common::*;

#[test]
fn write_to_string_when_constant_expression_initializer_then_round_trips() {
    let source = "
VAR_GLOBAL CONSTANT
    SCALE : LREAL := 2.5;
END_VAR
PROGRAM main
VAR
    scaled : LREAL := SCALE/180.5;
END_VAR
END_PROGRAM
";
    let options = CompilerOptions {
        allow_top_level_var_global: true,
        allow_constant_initializer_expressions: true,
        ..CompilerOptions::default()
    };
    assert_round_trips(source, &options);
}

#[test]
fn write_to_string_when_initializer_is_bare_constant_name_then_round_trips() {
    // One declaration per block: the renderer writes each declaration in a
    // block of its own, so a shared block would not re-parse to the same AST.
    let source = "
TYPE
    Color : (Red, Green);
END_TYPE
TYPE
    S : STRUCT
        f : UDINT := C;
    END_STRUCT;
END_TYPE
VAR_GLOBAL CONSTANT
    C : UDINT := 1;
END_VAR
VAR_GLOBAL CONSTANT
    D : UDINT := C;
END_VAR
VAR_GLOBAL
    g : Color := Green;
END_VAR
VAR_GLOBAL
    h : Color := Color#Red;
END_VAR
FUNCTION_BLOCK FB_A
VAR
    x : UDINT := C;
END_VAR
VAR
    y AT %MD0 : UDINT := D;
END_VAR
END_FUNCTION_BLOCK
";
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    assert_round_trips(source, &options);
}
