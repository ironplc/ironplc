//! Tests of assignments of calls of user-defined functions with outputs
//! compiled from the lowered program (REQ-LOW-lowering-092,
//! REQ-LOW-lowering-172, REQ-LOW-lowering-174), with every statement
//! required to lower. A program
//! with only scalar outputs is also checked to leave every variable as the
//! route that reads the AST leaves it after one scan; that route does not
//! read a string output.

use ironplc_codegen::Route;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::error::Trap;

use rstest::rstest;

use crate::common::{compile_by_route, parse, run_one_scan};
use crate::compile_route_lowered::{assert_same_code, run_lowered, run_lowered_agreeing};

/// A function whose outputs are its input plus one, whether that is
/// positive, a `REAL` and a count of the calls of the output it writes
/// twice.
const FUNCTION: &str = "
    FUNCTION F : DINT
      VAR_INPUT a : DINT; END_VAR
      VAR_OUTPUT q : DINT; done : BOOL; r : REAL; twice : DINT; END_VAR
      q := a + 1;
      done := q > 0;
      r := 1.5;
      twice := q;
      twice := twice * 2;
      F := a * 10;
    END_FUNCTION";

/// REQ-LOW-lowering-092: a call stores each output in its target once the
/// function returns: a negated one inverted, one of a narrower type
/// converted to its target's, and one in an element of an array. The result
/// is stored, converted, in the assignment's target, and an output the call
/// does not assign is left alone. The function's body writes and reads its
/// outputs.
#[test]
fn run_when_function_with_outputs_called_then_outputs_and_result_stored() {
    let snapshot = run_lowered_agreeing(&format!(
        "{FUNCTION}
         PROGRAM main
           VAR d : DINT := 4; b : BOOL := TRUE; lr : LREAL; l : LREAL; t : DINT;
               arr : ARRAY[1..2] OF DINT; k : DINT := 1; element : DINT; m : LINT; END_VAR
           l := F(a := d, q => d, NOT done => b, r => lr, twice => arr[2]);
           m := F(a := 2, twice => arr[k]);
           t := arr[2];
           element := arr[1];
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("d"), 5);
    assert_eq!(snapshot.read_as::<i32>("b"), 0);
    assert_eq!(snapshot.read_as::<f64>("lr"), 1.5);
    assert_eq!(snapshot.read_as::<f64>("l"), 40.0);
    assert_eq!(snapshot.read_as::<i32>("t"), 10);
    assert_eq!(snapshot.read_as::<i64>("m"), 20);
    assert_eq!(snapshot.read_as::<i32>("element"), 6);
}

/// REQ-LOW-lowering-172: the outputs are stored before the result, as the
/// AST route stores them, so a variable that is both an output's target and
/// the assignment's ends with the result.
#[test]
fn run_when_output_target_is_assignment_target_then_result_last() {
    let snapshot = run_lowered_agreeing(&format!(
        "{FUNCTION}
         PROGRAM main VAR d : DINT := 4; END_VAR
           d := F(a := d, q => d);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("d"), 40);
}

/// A call inside an `IF` and a `FOR` runs each time its statement does, and
/// an output starts again at every call.
#[test]
fn run_when_function_with_outputs_called_in_loop_and_if_then_each_call() {
    let snapshot = run_lowered_agreeing(&format!(
        "{FUNCTION}
         PROGRAM main VAR i : DINT; sum : DINT; last : DINT; r : DINT; b : BOOL; END_VAR
           FOR i := 1 TO 3 DO
             r := F(a := i, q => last);
             sum := sum + last;
           END_FOR;
           IF sum > 0 THEN r := F(a := -1, done => b); END_IF;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("sum"), 9);
    assert_eq!(snapshot.read_as::<i32>("last"), 4);
    assert_eq!(snapshot.read_as::<i32>("r"), -10);
    assert_eq!(snapshot.read_as::<i32>("b"), 0);
}

/// A function that calls another and reads its outputs traps on either
/// route: inside a function's body the VM reaches no variable of another
/// function, and this backend's layout keeps an output in the called
/// function's own variable.
#[test]
fn run_when_function_reads_outputs_of_function_it_calls_then_both_routes_trap() {
    let source = format!(
        "{FUNCTION}
         FUNCTION G : DINT
           VAR_INPUT a : DINT; END_VAR
           VAR_OUTPUT p : DINT; END_VAR
           G := F(a := a, q => p);
         END_FUNCTION
         PROGRAM main VAR r : DINT; p : DINT; END_VAR
           r := G(a := 3, p => p);
         END_PROGRAM"
    );
    let options = CompilerOptions::default();
    let (library, context) = parse(&source, &options);
    for route in [Route::Ast, Route::RequireLowered] {
        let container = compile_by_route(&library, &context, &options, route).unwrap();
        let fault = run_one_scan(&container).expect_err("the scan traps");
        assert!(
            matches!(fault.trap, Trap::InvalidVariableIndex(_)),
            "{route:?}"
        );
    }
}

/// An assignment of a call of a function with outputs compiles to the same
/// instructions and constants by both routes: the call, the store of each
/// output and the store of the result.
#[rstest]
#[case::outputs("d := F(a := 2, q => d, NOT done => b, r => lr);")]
#[case::no_outputs_assigned("d := F(a := d);")]
#[case::converted_result("lr := F(a := 2, twice => d);")]
#[case::inside_if("IF b THEN d := F(a := d, q => d); END_IF;")]
fn compile_when_function_with_outputs_called_then_same_code_as_ast(#[case] statement: &str) {
    assert_same_code(&format!(
        "{FUNCTION}
         PROGRAM main VAR d : DINT; b : BOOL; lr : LREAL; END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// REQ-LOW-lowering-172, REQ-LOW-lowering-174: calls with outputs inside
/// an assignment's value run, and store their outputs, in the order the
/// value evaluates them: two summed, one nested in another's input, one
/// followed by a constant and one negated. The output the first of two
/// calls stores is the input of the second, and the assignment's target,
/// which the result overwrites.
#[test]
fn run_when_calls_with_outputs_inside_expression_then_in_order() {
    let snapshot = run_lowered_agreeing(&format!(
        "{FUNCTION}
         PROGRAM main
           VAR r1 : DINT; r2 : DINT; r3 : DINT; r4 : DINT; d : DINT := 1;
               q1 : DINT; q2 : DINT; q3 : DINT; q4 : DINT; q5 : DINT; t : DINT; END_VAR
           r1 := F(a := 1, q => q1) + F(a := 2, q => q2);
           r2 := F(a := F(a := 1, q => q3), q => q4);
           r3 := F(a := 3, q => q5) + 1;
           r4 := -F(a := 4, twice => t);
           d := F(a := d, q => d) + F(a := d, q => d);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r1"), 30);
    assert_eq!(snapshot.read_as::<i32>("q1"), 2);
    assert_eq!(snapshot.read_as::<i32>("q2"), 3);
    assert_eq!(snapshot.read_as::<i32>("r2"), 100);
    assert_eq!(snapshot.read_as::<i32>("q3"), 2);
    assert_eq!(snapshot.read_as::<i32>("q4"), 11);
    assert_eq!(snapshot.read_as::<i32>("r3"), 31);
    assert_eq!(snapshot.read_as::<i32>("q5"), 4);
    assert_eq!(snapshot.read_as::<i32>("r4"), -40);
    assert_eq!(snapshot.read_as::<i32>("t"), 10);
    assert_eq!(snapshot.read_as::<i32>("d"), 30);
}

/// REQ-LOW-codegen-175: an assignment with calls with outputs inside its
/// value compiles to the same instructions and constants by both routes,
/// with no storage for a temporary: each call, the stores of its outputs,
/// then the rest of the value and the store of the result.
#[rstest]
#[case::sum("d := F(a := 1, q => d) + F(a := 2, q => k);")]
#[case::nested("d := F(a := F(a := 1, q => k), q => d);")]
#[case::then_constant("d := F(a := 3, q => k) + 1;")]
#[case::negated("d := -F(a := 3, q => k);")]
#[case::compared("b := F(a := 1, q => k) > F(a := 2, q => d);")]
fn compile_when_calls_with_outputs_inside_expression_then_same_code_as_ast(
    #[case] statement: &str,
) {
    assert_same_code(&format!(
        "{FUNCTION}
         PROGRAM main VAR d : DINT; k : DINT; b : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// REQ-LOW-lowering-092: a string output is stored in its target as a
/// string is, at the target's capacity, once the function returns; it
/// starts empty at every call. The route that reads the AST does not read a
/// string output (`compile_not_implemented_location.rs`), so the lowered
/// route is the only one that compiles the program.
#[test]
fn run_when_function_has_string_outputs_then_stored() {
    let source = "
        FUNCTION NAMED : BOOL
          VAR_INPUT n : DINT; END_VAR
          VAR_OUTPUT s : STRING[8]; w : WSTRING; END_VAR
          IF n > 0 THEN
            s := 'abcdefgh';
            w := \"de\";
          END_IF;
          NAMED := n > 0;
        END_FUNCTION
        PROGRAM main
          VAR ok : BOOL; t : STRING[4]; u : WSTRING; t2 : STRING := 'kept'; len : DINT; END_VAR
          ok := NAMED(n := 1, s => t, w => u);
          ok := NAMED(n := 0, s => t2);
          len := LEN(t);
        END_PROGRAM";
    let snapshot = run_lowered(source);
    assert_eq!(snapshot.read_as::<i32>("ok"), 0);
    assert_eq!(snapshot.read_as::<i32>("len"), 4);
    assert_eq!(snapshot.read("t"), "abcd");
    assert_eq!(snapshot.read("u"), "de");
    assert_eq!(snapshot.read("t2"), "");
}
