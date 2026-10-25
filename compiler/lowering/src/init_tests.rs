//! Tests of lowering `init`: the store of each global's and each program
//! instance field's starting value, through the place that holds it.

use crate::test_support::lower_whole;
use crate::PartialProgram;

/// One task running the instance `inst` of the program `main`, and the
/// globals `globals` declare, if any.
fn whole(globals: &str, source: &str) -> PartialProgram {
    let globals = if globals.is_empty() {
        String::new()
    } else {
        format!("VAR_GLOBAL {globals} END_VAR")
    };
    lower_whole(&format!(
        "{source}
CONFIGURATION config
  {globals}
  RESOURCE res ON PLC
    TASK t(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst WITH t : main;
  END_RESOURCE
END_CONFIGURATION
"
    ))
}

/// The text of each statement of `init`.
fn init(program: &PartialProgram) -> Vec<String> {
    program
        .init()
        .expect("init lowers")
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// REQ-LOW-lowering-194: each global, in the execution model's order, then
/// each field of the program instance, in declaration order, is stored the
/// value it starts with, a zero included.
#[test]
fn lower_program_when_scalars_then_init_stores_each_in_order() {
    let program = whole(
        "g : DINT := 5; h : DINT;",
        "
PROGRAM main
  VAR_EXTERNAL g : DINT; END_VAR
  VAR
    i : INT := -2;
    b : BOOL := TRUE;
    r : REAL;
    e : (Red, Green) := Green;
    d : TIME := T#1s;
  END_VAR
  i := 1;
END_PROGRAM
",
    );

    assert_eq!(
        init(&program),
        [
            "assign v0 (const i32s 5)",
            "assign v1 (const i32s 0)",
            "assign v3.f0 (const i32s -2)",
            "assign v3.f1 (const bool true)",
            "assign v3.f2 (const f32 0.0)",
            "assign v3.f3 (const i32s 1)",
            "assign v3.f4 (const i32s 1000)",
        ]
    );
}

/// REQ-LOW-lowering-194: an array is stored element by element, in
/// storage order, the last subscript varying fastest; a string with
/// `AssignStr`.
#[test]
fn lower_program_when_arrays_and_strings_then_init_stores_each_element() {
    let program = whole(
        "",
        "
PROGRAM main
  VAR
    a : ARRAY[1..2, 0..1] OF INT := [1, 2, 3, 4];
    s : STRING[4] := 'ab';
    ss : ARRAY[0..1] OF STRING[2] := ['x', 'y'];
  END_VAR
  a[1, 0] := 1;
END_PROGRAM
",
    );

    assert_eq!(
        init(&program),
        [
            "assign v1.f0[(const i32s 1), (const i32s 0)] (const i32s 1)",
            "assign v1.f0[(const i32s 1), (const i32s 1)] (const i32s 2)",
            "assign v1.f0[(const i32s 2), (const i32s 0)] (const i32s 3)",
            "assign v1.f0[(const i32s 2), (const i32s 1)] (const i32s 4)",
            "assign-str v1.f1 (literal string[2] \"ab\")",
            "assign-str v1.f2[(const i32s 0)] (literal string[1] \"x\")",
            "assign-str v1.f2[(const i32s 1)] (literal string[1] \"y\")",
        ]
    );
}

/// REQ-LOW-lowering-194: a structure, an array of structures and a function
/// block instance are stored field by field. An instance's own initializer,
/// and a nested instance's, are part of the value it starts with.
#[test]
fn lower_program_when_structures_and_instances_then_init_stores_each_field() {
    let program = whole(
        "",
        "
TYPE Pair : STRUCT n : INT := 1; w : WSTRING[3] := \"hi\"; END_STRUCT; END_TYPE
FUNCTION_BLOCK Inner
  VAR_INPUT k : INT := 4; END_VAR
  k := k;
END_FUNCTION_BLOCK
FUNCTION_BLOCK Outer
  VAR_INPUT j : INT := 2; END_VAR
  VAR n : Inner := (k := 6); END_VAR
  n();
END_FUNCTION_BLOCK
PROGRAM main
  VAR
    p : Pair := (n := 3);
    ps : ARRAY[0..1] OF Pair;
    o : Outer := (j := 7);
  END_VAR
  o();
END_PROGRAM
",
    );

    assert_eq!(
        init(&program),
        [
            "assign v3.f0.f0 (const i32s 3)",
            "assign-str v3.f0.f1 (literal wstring[2] \"hi\")",
            "assign v3.f1[(const i32s 0)].f0 (const i32s 1)",
            "assign-str v3.f1[(const i32s 0)].f1 (literal wstring[2] \"hi\")",
            "assign v3.f1[(const i32s 1)].f0 (const i32s 1)",
            "assign-str v3.f1[(const i32s 1)].f1 (literal wstring[2] \"hi\")",
            "assign v3.f2.f0 (const i32s 7)",
            "assign v3.f2.f1.f0 (const i32s 6)",
        ]
    );
}

/// REQ-LOW-lowering-194: a reference starts `NULL`, or refers to the
/// variable `REF` names, reached through the program instance rather than
/// the body's instance parameter.
#[test]
fn lower_program_when_references_then_init_stores_null_or_ref_through_instance() {
    let program = whole(
        "",
        "
PROGRAM main
  VAR x : INT := 2; r : REF_TO INT := REF(x); n : REF_TO INT; END_VAR
  x := 1;
END_PROGRAM
",
    );

    assert_eq!(
        init(&program),
        [
            "assign v1.f0 (const i32s 2)",
            "assign v1.f1 (ref_to v1.f0)",
            "assign v1.f2 (null)",
        ]
    );
}

/// REQ-LOW-lowering-194: a member initializer that is an expression is
/// stored as an assignment does, the program's variables it reads reached
/// through the program instance rather than the body's instance parameter.
#[test]
fn lower_program_when_member_initializer_is_expression_then_reads_through_instance() {
    let program = whole(
        "",
        "
FUNCTION_BLOCK Fb
  VAR_INPUT k : INT; END_VAR
  k := k;
END_FUNCTION_BLOCK
PROGRAM main
  VAR x : INT := 2; f : Fb := (k := x + 1); END_VAR
  f();
END_PROGRAM
",
    );

    assert_eq!(
        init(&program),
        [
            "assign v2.f0 (const i32s 2)",
            "assign v2.f1.f0 (truncate 16 (add i32s (read i32s v2.f0) (const i32s 1)))",
        ]
    );
}
