//! General expressions as struct/FB-instance initializer values, e.g.
//! `tonDelta : TON := (PT := pDevice^.Delta);`. The parser accepts the value
//! expression unconditionally (a permissive superset); the
//! `--allow-struct-initializer-expressions` flag is enforced by a later
//! semantic rule, not here.

use super::common::*;
use dsl::common::StructInitialValueAssignmentKind;

#[test]
fn parse_when_struct_init_value_is_deref_member_expr_then_parses_as_expression() {
    // Real motivating shape: a call-style FB-instance initializer whose
    // value is a genuinely runtime expression (dereference + member
    // access), not a compile-time constant.
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
END_FUNCTION_BLOCK";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(source, &FileId::default(), &options).unwrap();

    let fb = cast!(
        &library.elements[1],
        LibraryElementKind::FunctionBlockDeclaration
    );
    let struct_init = cast!(
        &fb.variables[1].initializer,
        InitialValueAssignmentKind::Structure
    );
    assert_eq!(struct_init.elements_init.len(), 1);
    assert!(matches!(
        struct_init.elements_init[0].init,
        StructInitialValueAssignmentKind::Expression(_)
    ));
}
