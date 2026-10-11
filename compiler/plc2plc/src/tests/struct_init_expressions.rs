//! Round-tripping of struct/FB-instance initializer values: general
//! expressions, e.g. `tonDelta : TON := (PT := pDevice^.Delta);`, and
//! nested arrays and structures.

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

/// A member that is itself an array or a structure renders in brackets or
/// parentheses.
#[test]
fn write_to_string_when_struct_init_member_is_array_or_structure_then_round_trips() {
    let source = "
TYPE
    Inner : STRUCT x : INT; END_STRUCT;
    Outer : STRUCT values : ARRAY[1..2] OF INT; inner : Inner; END_STRUCT;
END_TYPE

PROGRAM main
VAR
    o : Outer := (values := [1, 2], inner := (x := 3));
END_VAR
END_PROGRAM
";
    let rendered = assert_round_trips(source, &CompilerOptions::default());

    assert_eq!(
        rendered,
        "TYPE
   Inner : STRUCT
      x : INT ;
   END_STRUCT ;
END_TYPE
TYPE
   Outer : STRUCT
      values : ARRAY [ 1.. 2 ] OF INT ;
      inner : Inner ;
   END_STRUCT ;
END_TYPE
PROGRAM main

VAR
   o : Outer := ( values := [ 1 , 2 ] , inner := ( x := 3 ) );
END_VAR
END_PROGRAM
"
    );
}

/// After analysis, a function block instance's member initializer is a
/// function block initializer rather than a late-resolved one; its members
/// still render.
#[test]
fn write_to_string_when_analyzed_fb_instance_has_member_initializer_then_renders_members() {
    let source = "
FUNCTION_BLOCK Counter
VAR_INPUT
    preset : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
    counter : Counter := (preset := 5);
END_VAR
END_PROGRAM
";
    let options = CompilerOptions::default();
    let (library, rendered) = analyze_and_render_with_types(source, &options);

    assert_eq!(
        rendered,
        "FUNCTION_BLOCK Counter

   VAR_INPUT
      preset : int;
   END_VAR

END_FUNCTION_BLOCK
PROGRAM main

VAR
   counter : Counter := ( preset := 5 );
END_VAR
END_PROGRAM
"
    );
    assert_annotated_reparses_as_plain(&library, &rendered, &options);
}
