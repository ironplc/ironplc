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
    let rendered = assert_round_trips(source, &options);

    assert!(rendered.contains("SCALE"));
    assert!(rendered.contains("180.5"));
}
