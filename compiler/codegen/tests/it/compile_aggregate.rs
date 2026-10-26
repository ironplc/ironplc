//! Tests of whole arrays and structures assigned from the lowered program.

use rstest::rstest;

use crate::common::{compile_analyzed, parse};

use crate::compile_body::{assert_compiles, compile_and_run, compile_and_run_agreeing, options};

/// The declarations the tests copy between.
const TYPES: &str = "
    TYPE S : STRUCT n : DINT; a : ARRAY[1..2] OF INT; END_STRUCT; END_TYPE";

/// A copy leaves the destination with the source's values and its own
/// storage: a write to the destination afterwards leaves the source as it
/// was.
#[test]
fn run_when_arrays_and_structures_copied_then_destination_holds_values() {
    let snapshot = compile_and_run(&format!(
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

/// A whole array of structures is copied by value, string fields included:
/// a write to the source afterwards leaves the destination as it was.
#[test]
fn run_when_array_of_structures_copied_then_destination_holds_values() {
    let snapshot = compile_and_run(
        "TYPE P : STRUCT x : DINT; n : STRING[8]; END_STRUCT; END_TYPE
         PROGRAM main
           VAR ps : ARRAY[0..2] OF P; qs : ARRAY[0..2] OF P;
               copied : DINT; later : DINT; name : STRING[8]; END_VAR
           qs[1].x := 5;
           qs[1].n := 'abc';
           ps := qs;
           qs[1].x := 7;
           qs[1].n := 'zz';
           copied := ps[1].x;
           later := qs[1].x;
           name := ps[1].n;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("copied"), 5);
    assert_eq!(snapshot.read_as::<i32>("later"), 7);
    assert_eq!(snapshot.read("name"), "abc");
}

/// A copy lowers and compiles.
#[rstest]
#[case::array("x := y;")]
#[case::array_2d("m := n;")]
#[case::string_array("s := t;")]
#[case::structure("a := b;")]
#[case::itself("x := x;")]
fn compile_when_copy_lowered_then_compiles(#[case] statement: &str) {
    assert_compiles(&format!(
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

/// A copy to and from a global reached through `VAR_EXTERNAL` lowers and
/// compiles.
#[rstest]
#[case::to_global("g := a;")]
#[case::from_global("a := g;")]
fn compile_when_copy_of_external_lowered_then_compiles(#[case] statement: &str) {
    assert_compiles(&format!(
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

/// A copy that is not a region copy is not emitted from the lowered program,
/// so it is not supported yet (P9999): an element target, a block's output as
/// the source, and a method's own instance into a field of the block that
/// holds one, which has storage of its own (REQ-LOW-codegen-196).
#[rstest]
#[case::element_target(
    "PROGRAM main VAR arr : ARRAY[1..2] OF S; one : S; END_VAR arr[1] := one; END_PROGRAM"
)]
#[case::block_output_source(
    "FUNCTION_BLOCK B VAR_OUTPUT p : S; END_VAR p.n := 1; END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; r : S; END_VAR r := f.p; END_PROGRAM"
)]
#[case::method_instance_into_held_field(
    "FUNCTION_BLOCK INNER VAR_INPUT v : DINT; END_VAR END_FUNCTION_BLOCK
     FUNCTION_BLOCK B VAR_OUTPUT ob : INNER; END_VAR
       METHOD M : DINT VAR loc : INNER; END_VAR ob := loc; M := 1; END_METHOD
     END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; k : DINT; END_VAR k := f.M(); END_PROGRAM"
)]
fn compile_when_copy_not_emitted_then_not_implemented(#[case] pous: &str) {
    let options = options();
    let (library, context) = parse(&format!("{TYPES}\n{pous}"), &options);
    let error = compile_analyzed(&library, &context, &options).unwrap_err();
    assert_eq!(error.code, "P9999");
    assert_eq!(
        error.primary.message,
        "Statement code generation does not support yet"
    );
}

/// A block whose instances the body copies hold.
const INNER: &str = "
    FUNCTION_BLOCK INNER
      VAR_INPUT v : DINT; END_VAR
      VAR_OUTPUT q : DINT; END_VAR
      q := v;
    END_FUNCTION_BLOCK";

/// A whole structure, array or instance copied between variables of a
/// function, function block or method body, which this backend's layout
/// keeps no region for, or from a global, lowers and compiles: the
/// destination's slot takes the source's. This backend compiles no read
/// inside such a variable there, and no call of such an instance, so no
/// program tells this from a copy (REQ-LOW-codegen-165).
#[rstest]
#[case::function_locals(
    "FUNCTION FC : DINT VAR lp : S; lq : S; END_VAR lp := lq; FC := 1; END_FUNCTION
     PROGRAM main VAR k : DINT; END_VAR k := FC(); END_PROGRAM"
)]
#[case::block_fields(
    "FUNCTION_BLOCK B VAR x : S; y : S; END_VAR x := y; END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; END_VAR f(); END_PROGRAM"
)]
#[case::block_array_fields(
    "FUNCTION_BLOCK B VAR x : ARRAY[0..2] OF DINT; y : ARRAY[0..2] OF DINT; END_VAR
       x := y;
     END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; END_VAR f(); END_PROGRAM"
)]
#[case::method_local_from_field(
    "FUNCTION_BLOCK B VAR fp : S; END_VAR
       METHOD M : DINT VAR lp : S; END_VAR lp := fp; M := 1; END_METHOD
     END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; k : DINT; END_VAR k := f.M(); END_PROGRAM"
)]
#[case::method_instance_locals(
    "FUNCTION_BLOCK B
       METHOD M : DINT VAR x : INNER; y : INNER; END_VAR x := y; M := 1; END_METHOD
     END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; k : DINT; END_VAR k := f.M(); END_PROGRAM"
)]
#[case::from_global(
    "FUNCTION_BLOCK B VAR_EXTERNAL g : S; END_VAR VAR lp : S; END_VAR lp := g; END_FUNCTION_BLOCK
     PROGRAM main VAR f : B; END_VAR f(); END_PROGRAM
     CONFIGURATION config
       VAR_GLOBAL g : S; END_VAR
       RESOURCE res ON PLC
         TASK plc_task(INTERVAL := T#100MS, PRIORITY := 1);
         PROGRAM plc_task_instance WITH plc_task : main;
       END_RESOURCE
     END_CONFIGURATION"
)]
fn compile_when_copy_in_body_lowered_then_compiles(#[case] pous: &str) {
    assert_compiles(&format!("{TYPES}\n{INNER}\n{pous}"));
}

/// Copies in a function's and a block's body leave every variable with the
/// same values on every backend, the function's result and the block's
/// output included.
#[test]
fn run_when_copy_in_body_then_backends_agree() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{TYPES}
         {INNER}
         FUNCTION FC : DINT VAR lp : S; lq : S; END_VAR lp := lq; FC := 3; END_FUNCTION
         FUNCTION_BLOCK B
           VAR x : S; y : S; END_VAR
           VAR_OUTPUT done : DINT; END_VAR
           x := y;
           done := done + 1;
         END_FUNCTION_BLOCK
         PROGRAM main VAR f : B; k : DINT; d : DINT; END_VAR
           k := FC();
           f();
           d := f.done;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("k"), 3);
    assert_eq!(snapshot.read_as::<i32>("d"), 1);
}
