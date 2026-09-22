//! Round-tripping of general expressions used as struct/FB-instance
//! initializer values, e.g. `tonDelta : TON := (PT := pDevice^.Delta);`.

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
    assert_round_trips(source, &options);
}

/// A bare variable as the value round-trips too. The renderer works on the
/// parse tree, where it is still an enumerated value; the analyzer is what
/// later reclassifies it, so both spellings must render the same text back.
#[test]
fn write_to_string_when_struct_init_value_is_bare_identifier_then_round_trips() {
    let source = "
TYPE MyStruct :
STRUCT
    x : INT;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    g : INT;
    s : MyStruct := (x := g);
END_VAR
END_PROGRAM
";
    assert_round_trips(source, &CompilerOptions::default());
}

/// A function block instance declared with member initial values.
#[test]
fn write_to_string_when_fb_instance_has_member_initializer_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    tonDelta : TON := (PT := T#100MS);
END_VAR
END_FUNCTION_BLOCK
";
    assert_round_trips(source, &CompilerOptions::default());
}
