//! Tests of calls of user-defined functions compiled from the lowered
//! program, with every statement required to lower.

use rstest::rstest;

use crate::compile_route_lowered::{assert_same_code, run_lowered};

/// Functions the tests call: one of three `DINT`s, two returning an `INT`,
/// one that calls another, and one whose local starts from its initial value
/// at every call.
const FUNCTIONS: &str = "
    FUNCTION ADD3 : DINT VAR_INPUT a : DINT; b : DINT; c : DINT; END_VAR
      ADD3 := a + b + c;
    END_FUNCTION
    FUNCTION HALF : INT VAR_INPUT a : INT; END_VAR
      HALF := a / 2;
    END_FUNCTION
    FUNCTION SCALE : INT VAR_INPUT x : LREAL; k : DINT; END_VAR
      SCALE := LREAL_TO_INT(x * DINT_TO_LREAL(k));
    END_FUNCTION
    FUNCTION TWICE : DINT VAR_INPUT a : DINT; END_VAR
      TWICE := ADD3(a, a, 0);
    END_FUNCTION
    FUNCTION COUNT : DINT VAR n : DINT; END_VAR
      n := n + 1;
      COUNT := n;
    END_FUNCTION";

/// A call passes its arguments in the order the function declares them,
/// whatever order the call names them in, and converts an argument of
/// another type to its parameter's. A function's result narrows where it is
/// stored, a function calls another, and a local starts again at every call.
#[test]
fn run_when_user_functions_called_then_values() {
    let snapshot = run_lowered(&format!(
        "{FUNCTIONS}
         PROGRAM main
           VAR s : SINT := -2; r1 : DINT; r2 : INT; r3 : DINT; r4 : DINT; r5 : DINT;
               r6 : INT; END_VAR
           r1 := ADD3(c := 100, a := 1, b := 20);
           r2 := SCALE(k := 2, x := 1.5);
           r3 := ADD3(ADD3(1, 2, 3), s, 4) * 10;
           r4 := TWICE(21);
           r5 := COUNT() + COUNT();
           r6 := SCALE(20000.0, 2);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r1"), 121);
    assert_eq!(snapshot.read_as::<i32>("r2"), 3);
    assert_eq!(snapshot.read_as::<i32>("r3"), 80);
    assert_eq!(snapshot.read_as::<i32>("r4"), 42);
    assert_eq!(snapshot.read_as::<i32>("r5"), 2);
    assert_eq!(snapshot.read_as::<i32>("r6"), -25536);
}

/// A call of a user-defined function compiles to the same instructions and
/// constants by both routes. Only the functions a program calls are
/// compiled, so `SCALE` is left out: its `LREAL_TO_INT`, stored to an `INT`,
/// the AST route narrows twice.
#[rstest]
#[case::positional("d := ADD3(d, e, 1);")]
#[case::named_out_of_order("d := ADD3(c := d, a := e, b := 1);")]
#[case::converted_argument("d := ADD3(s, d, s);")]
#[case::narrow_result("i := HALF(i) + HALF(s);")]
#[case::nested("d := ADD3(ADD3(d, e, 1), TWICE(e), COUNT()) + 1;")]
fn compile_when_user_function_call_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main
           VAR d : DINT; e : DINT; s : SINT; i : INT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// Functions with a `VAR_IN_OUT`: one adding to the variable a call passes,
/// one passing its own on, and one swapping two.
const IN_OUT_FUNCTIONS: &str = "
    FUNCTION INC : DINT VAR_IN_OUT v : DINT; END_VAR VAR_INPUT n : DINT; END_VAR
      v := v + n;
      INC := v;
    END_FUNCTION
    FUNCTION INC_TWICE : DINT VAR_IN_OUT w : DINT; END_VAR
      INC_TWICE := INC(w, 1) + INC(w, 1);
    END_FUNCTION
    FUNCTION SWAP : BOOL VAR_IN_OUT a : DINT; b : DINT; END_VAR VAR t : DINT; END_VAR
      t := a;
      a := b;
      b := t;
      SWAP := TRUE;
    END_FUNCTION";

/// A function reads and writes the variable a call passes for its
/// `VAR_IN_OUT`, and passes on one it was passed (REQ-LOW-lowering-033).
#[test]
fn run_when_in_out_passed_then_callee_writes_callers_variable() {
    let snapshot = run_lowered(&format!(
        "{IN_OUT_FUNCTIONS}
         PROGRAM main
           VAR x : DINT := 5; r1 : DINT; r2 : DINT; i : DINT := 1; j : DINT := 2;
               ok : BOOL; END_VAR
           r1 := INC(v := x, n := 2);
           r2 := INC_TWICE(x);
           ok := SWAP(i, j);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r1"), 7);
    assert_eq!(snapshot.read_as::<i32>("r2"), 17);
    assert_eq!(snapshot.read_as::<i32>("x"), 9);
    assert_eq!(snapshot.read_as::<i32>("i"), 2);
    assert_eq!(snapshot.read_as::<i32>("j"), 1);
    assert_eq!(snapshot.read_as::<i32>("ok"), 1);
}

/// Bits written to a function's `VAR_IN_OUT` are written to the caller's
/// variable through its reference, leaving the other bits as they were. The
/// AST route reports the write as not implemented.
#[test]
fn run_when_in_out_has_bits_written_then_callers_variable() {
    let snapshot = run_lowered(
        "FUNCTION SET0 : BOOL VAR_IN_OUT w : WORD; END_VAR
           w.0 := TRUE;
           SET0 := w.1;
         END_FUNCTION
         PROGRAM main
           VAR m : WORD := 16#0102; r : BOOL; END_VAR
           r := SET0(m);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<u64>("m"), 0x0103);
    assert_eq!(snapshot.read_as::<u64>("r"), 1);
}

/// A call of a function with a `VAR_IN_OUT`, and the function's body,
/// compile to the same instructions and constants by both routes: the call
/// passes a variable's index, or the reference a `VAR_IN_OUT` of the caller
/// holds, and the body loads and stores through it.
#[rstest]
#[case::variable("d := INC(v := d, n := e);")]
#[case::passed_on("d := INC_TWICE(e);")]
#[case::two_in_outs("x := SWAP(i, k);")]
fn compile_when_in_out_call_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "{IN_OUT_FUNCTIONS}
         PROGRAM main
           VAR d : DINT; e : DINT; i : DINT; k : DINT; x : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}
