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
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("SCALE"));
    assert!(rendered.contains("180.5"));

    let library_rendered = parse_program(&rendered, &FileId::default(), &options)
        .expect("rendered output must parse under the same dialect");
    assert_eq!(library_original, library_rendered);
}
