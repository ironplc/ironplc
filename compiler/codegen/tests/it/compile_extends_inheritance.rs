//! What a derived function block inherits reaches codegen but is not
//! compiled yet: `compile_user_function_block` lays out a function
//! block's own fields only, never flattened with the fields it inherits
//! through `EXTENDS`, so an inherited field has no storage and an
//! inherited method body has nothing to run against.
//!
//! Analysis accepts both -- `xform_resolve_expr_types` resolves an
//! inherited field and `rule_method_call_declared` resolves a call up
//! the `EXTENDS` chain -- so codegen is where they stop. These tests pin
//! that boundary, which
//! `docs/reference/language/object-orientation/extends.rst` states.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;

fn opts_with_fb_inheritance() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn compile_when_derived_body_reads_inherited_field_then_variable_undefined() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    running : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor
VAR
    speed : INT;
END_VAR
    running := TRUE;
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_AdvancedMotor;
END_VAR
    m();
END_PROGRAM
";

    let result = try_parse_and_compile(source, &opts_with_fb_inheritance());

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, "P4007");
}

#[test]
fn compile_when_call_to_inherited_method_then_not_implemented() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    running : BOOL;
END_VAR
METHOD Start
    running := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor
VAR
    speed : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_AdvancedMotor;
END_VAR
    m.Start();
END_PROGRAM
";

    let result = try_parse_and_compile(source, &opts_with_fb_inheritance());

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, "P9999");
}
