//! Tests of whole arrays and structures assigned from the lowered program,
//! with every statement required to lower.

use ironplc_codegen::Route;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{compile_by_route, parse};

use crate::compile_route_lowered::{assert_same_code, run_lowered};

/// The declarations the tests copy between.
const TYPES: &str = "
    TYPE S : STRUCT n : DINT; a : ARRAY[1..2] OF INT; END_STRUCT; END_TYPE";

/// A copy leaves the destination with the source's values and its own
/// storage: a write to the destination afterwards leaves the source as it
/// was.
#[test]
fn run_when_arrays_and_structures_copied_then_destination_holds_values() {
    let snapshot = run_lowered(&format!(
        "{TYPES}
         PROGRAM main
           VAR x : ARRAY[1..3] OF DINT; y : ARRAY[1..3] OF DINT;
               m : ARRAY[1..2, 1..3] OF INT; n : ARRAY[1..2, 1..3] OF INT;
               a : S; b : S;
               r1 : DINT; r2 : DINT; r3 : INT; r4 : DINT; r5 : INT; r6 : INT; END_VAR
           y[2] := 7;
           x := y;
           x[2] := 8;
           r1 := x[2];
           r2 := y[2];
           n[2, 3] := 5;
           m := n;
           r3 := m[2, 3];
           b.n := 11;
           b.a[2] := 12;
           a := b;
           b.a[2] := 13;
           r4 := a.n;
           r5 := a.a[2];
           r6 := b.a[2];
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r1"), 8);
    assert_eq!(snapshot.read_as::<i32>("r2"), 7);
    assert_eq!(snapshot.read_as::<i32>("r3"), 5);
    assert_eq!(snapshot.read_as::<i32>("r4"), 11);
    assert_eq!(snapshot.read_as::<i32>("r5"), 12);
    assert_eq!(snapshot.read_as::<i32>("r6"), 13);
}

/// A copy compiles to the same instructions and constants by both routes.
#[rstest]
#[case::array("x := y;")]
#[case::array_2d("m := n;")]
#[case::string_array("s := t;")]
#[case::structure("a := b;")]
#[case::itself("x := x;")]
fn compile_when_copy_lowered_then_same_code_as_ast(#[case] statement: &str) {
    assert_same_code(&format!(
        "{TYPES}
         PROGRAM main
           VAR x : ARRAY[1..3] OF DINT; y : ARRAY[1..3] OF DINT;
               m : ARRAY[1..2, 1..3] OF INT; n : ARRAY[1..2, 1..3] OF INT;
               s : ARRAY[1..2] OF STRING[8]; t : ARRAY[1..2] OF STRING[8];
               a : S; b : S; END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// A copy to and from a global reached through `VAR_EXTERNAL` compiles to
/// the same instructions and constants by both routes.
#[rstest]
#[case::to_global("g := a;")]
#[case::from_global("a := g;")]
fn compile_when_copy_of_external_lowered_then_same_code_as_ast(#[case] statement: &str) {
    assert_same_code(&format!(
        "CONFIGURATION config
           VAR_GLOBAL g : ARRAY[1..2] OF DINT; END_VAR
           RESOURCE res ON PLC
             TASK t (INTERVAL := T#100ms, PRIORITY := 1);
             PROGRAM p1 WITH t : main;
           END_RESOURCE
         END_CONFIGURATION
         PROGRAM main
           VAR_EXTERNAL g : ARRAY[1..2] OF DINT; END_VAR
           VAR a : ARRAY[1..2] OF DINT; END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// A copy the AST route does not compile as a region copy is not emitted
/// from the lowered program, so it is left to the AST route: an element
/// target, a block's output as the source, and fields of a block's own
/// instance, which this backend's layout keeps no region for in the block's
/// body.
#[rstest]
#[case::element_target(
    "PROGRAM main VAR arr : ARRAY[1..2] OF S; one : S; END_VAR arr[1] := one; END_PROGRAM"
)]
#[case::block_output_source(
    "FUNCTION_BLOCK B VAR_OUTPUT p : S; END_VAR p.n := 1; END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; r : S; END_VAR r := f.p; END_PROGRAM"
)]
#[case::block_fields(
    "FUNCTION_BLOCK B VAR x : S; y : S; END_VAR x := y; END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; END_VAR f(); END_PROGRAM"
)]
fn compile_when_copy_not_emitted_then_left_to_ast_route(#[case] pous: &str) {
    let options = CompilerOptions::default();
    let (library, context) = parse(&format!("{TYPES}\n{pous}"), &options);
    let error = compile_by_route(&library, &context, &options, Route::RequireLowered).unwrap_err();
    assert_eq!(error.code, "P9999");
    assert_eq!(
        error.primary.message,
        "Statement the lowered route does not support yet"
    );
}
