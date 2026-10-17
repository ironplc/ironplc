//! Tests of calls of user-defined functions whose result is a structure,
//! compiled from the lowered program (REQ-LOW-lowering-079,
//! REQ-LOW-lowering-186), with every statement required to lower. Each
//! program compiles to the same code as by the route that reads the AST, the
//! functions' bodies included, and leaves every variable and the data region
//! as that route leaves them after one scan.

use crate::compile_route_lowered::{assert_same_code, run_lowered_agreeing};

/// A structure; a function of an input and an output whose result is one,
/// which returns early for a negative input; a function of no parameters
/// whose result is one; and a function whose value is a `DINT`.
const FUNCTIONS: &str = "
    TYPE P : STRUCT x : DINT; y : REAL; END_STRUCT; END_TYPE
    FUNCTION MK : P
      VAR_INPUT v : DINT; END_VAR
      VAR_OUTPUT n : DINT; END_VAR
      MK.x := v;
      IF v < 0 THEN RETURN; END_IF;
      MK.y := 1.5;
      n := v + 1;
    END_FUNCTION
    FUNCTION ORIGIN : P
      ORIGIN.x := 0;
    END_FUNCTION
    FUNCTION TWO : DINT
      TWO := 2;
    END_FUNCTION";

/// REQ-LOW-lowering-186: a call of a function whose result is a structure,
/// assigned to a structure, stores its outputs, then copies its result to
/// the target, so a field of the target the call also stores an output in
/// takes the result's value. The call may take a call's value as an input,
/// have no parameters, or be in a branch.
#[test]
fn run_when_structure_result_assigned_then_outputs_stored_then_result_copied() {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main
           VAR p : P; q : P; o : P; k : DINT;
               px : DINT; py : REAL; qx : DINT; ox : DINT; END_VAR
           p := MK(v := 3, n => k);
           q := MK(v := TWO(), n => q.x);
           o := ORIGIN();
           IF k > 0 THEN o := MK(v := k); END_IF;
           px := p.x;
           py := p.y;
           qx := q.x;
           ox := o.x;
         END_PROGRAM"
    );
    assert_same_code(&source);
    let snapshot = run_lowered_agreeing(&source);
    assert_eq!(snapshot.read_as::<i32>("px"), 3);
    assert_eq!(snapshot.read_as::<f32>("py"), 1.5);
    assert_eq!(snapshot.read_as::<i32>("k"), 4);
    assert_eq!(snapshot.read_as::<i32>("qx"), 2);
    assert_eq!(snapshot.read_as::<i32>("ox"), 4);
}

/// A function that returns early leaves its result as its body set it
/// before the `RETURN`.
#[test]
fn run_when_structure_result_returned_early_then_copied_as_set() {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main
           VAR p : P; k : DINT; px : DINT; END_VAR
           p := MK(v := -1, n => k);
           px := p.x;
         END_PROGRAM"
    );
    assert_same_code(&source);
    let snapshot = run_lowered_agreeing(&source);
    assert_eq!(snapshot.read_as::<i32>("px"), -1);
}
