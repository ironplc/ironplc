//! Round-tripping of general expressions used as struct/FB-instance
//! initializer values, e.g. `tonDelta : TON := (PT := pDevice^.Delta);`.
//! See specs/plans/2026-07-26-twincat-struct-init-expression-value.md.

use super::common::*;

#[test]
fn write_to_string_when_struct_init_value_is_deref_member_expr_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Device
VAR_INPUT
    Delta : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    pDevice : REF_TO FB_Device;
    tonDelta : TON := (PT := pDevice^.Delta);
END_VAR
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("pDevice"));
    assert!(rendered.contains("Delta"));

    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}
