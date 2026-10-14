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
