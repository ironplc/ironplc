//! Tests of calls of user-defined functions whose result is a string,
//! compiled from the lowered program (REQ-LOW-lowering-079,
//! REQ-LOW-lowering-179), with every statement required to lower. Each
//! program compiles to the same code as by the route that reads the AST,
//! the functions' bodies included, and leaves every variable and the data
//! region as that route leaves them after one scan.

use ironplc_codegen::Route;

use crate::common::{compile_by_route, parse};
use crate::compile_route_lowered::{assert_same_code, options, run_lowered_agreeing};

/// Functions whose results are strings: a `STRING[20]` input doubled, the
/// same with a `!` after it, and a `WSTRING` input as it is.
const FUNCTIONS: &str = "
    FUNCTION TWICE : STRING[20]
      VAR_INPUT text : STRING[20]; END_VAR
      TWICE := CONCAT(text, text);
    END_FUNCTION
    FUNCTION DECORATE : STRING[32]
      VAR_INPUT v : STRING[32]; END_VAR
      DECORATE := CONCAT(v, '!');
    END_FUNCTION
    FUNCTION MK : WSTRING
      VAR_INPUT w : WSTRING[10]; END_VAR
      MK := w;
    END_FUNCTION";

/// REQ-LOW-lowering-079: the result reaches the variable it is assigned to
/// only once the call returns, so a call may read that variable; and a call
/// may be the argument of another.
#[test]
fn run_when_result_assigned_to_argument_or_nested_then_values() {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main
           VAR same : STRING[20] := 'ab'; nested : STRING[20]; END_VAR
           same := TWICE(same);
           nested := TWICE(TWICE('x'));
         END_PROGRAM"
    );
    assert_same_code(&source);
    let snapshot = run_lowered_agreeing(&source);
    assert_eq!(snapshot.read("same"), "abab");
    assert_eq!(snapshot.read("nested"), "xxxx");
}

/// REQ-LOW-lowering-179: a call is emitted where its result is read: two
/// calls of one function as the operands of `CONCAT`, a `WSTRING` result
/// after a literal operand, a result longer than the default capacity whose
/// length `LEN` takes, and one compared in an `IF` condition.
#[test]
fn run_when_result_is_operand_then_call_emitted_where_read() {
    let half = "h".repeat(128);
    let source = format!(
        "{FUNCTIONS}
         FUNCTION DOUBLED : STRING[300]
           VAR_INPUT half : STRING[128]; END_VAR
           DOUBLED := CONCAT(half, half);
         END_FUNCTION
         PROGRAM main
           VAR a : STRING[20] := 'a'; b : STRING[20] := 'b'; joined : STRING[40];
               w : WSTRING[10] := \"cd\"; wide : WSTRING[20];
               c128 : STRING[128] := '{half}'; n : INT; hit : DINT; END_VAR
           joined := CONCAT(TWICE(a), TWICE(b));
           wide := CONCAT(\"ab\", MK(w));
           n := LEN(DOUBLED(c128));
           IF TWICE(a) = 'aa' THEN hit := 1; END_IF;
         END_PROGRAM"
    );
    assert_same_code(&source);
    let snapshot = run_lowered_agreeing(&source);
    assert_eq!(snapshot.read("joined"), "aabb");
    assert_eq!(snapshot.read("wide"), "abcd");
    assert_eq!(snapshot.read_as::<i32>("n"), 256);
    assert_eq!(snapshot.read_as::<i32>("hit"), 1);
}

/// A call in a loop of 200 iterations hands its buffer back at every
/// iteration, so the pool of temporary buffers is the size the route that
/// reads the AST gives it.
#[test]
fn run_when_called_in_loop_then_buffers_released_and_pool_as_ast_route() {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main VAR t : STRING[32]; i : INT; n : DINT; END_VAR
           FOR i := 1 TO 200 DO
             t := DECORATE('hi');
             n := LEN(DECORATE(t)) + n;
           END_FOR;
         END_PROGRAM"
    );
    assert_same_code(&source);
    let snapshot = run_lowered_agreeing(&source);
    assert_eq!(snapshot.read("t"), "hi!");
    assert_eq!(snapshot.read_as::<i32>("n"), 800);
    let options = options();
    let (library, context) = parse(&source, &options);
    let header = |route| {
        let header = compile_by_route(&library, &context, &options, route)
            .expect("the program compiles")
            .header;
        (header.num_temp_bufs, header.max_temp_buf_bytes)
    };
    assert_eq!(header(Route::RequireLowered), header(Route::Ast));
}

/// REQ-LOW-lowering-180: a call in an `ELSIF` condition is in place there,
/// so it runs only when the conditions before it are false: it decides the
/// second statement and not the first. The code differs from the route that
/// reads the AST only in the jump that route emits after the last branch of
/// an `IF` with no `ELSE`.
#[test]
fn run_when_result_in_elsif_condition_then_lowered_in_place() {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main VAR a : STRING[20] := 'a'; n : DINT := 1; k : DINT; j : DINT; END_VAR
           IF n > 0 THEN k := 1; ELSIF TWICE(a) = 'aa' THEN k := 2; END_IF;
           IF n < 0 THEN j := 1; ELSIF TWICE(a) = 'aa' THEN j := 2; END_IF;
         END_PROGRAM"
    );
    let snapshot = run_lowered_agreeing(&source);
    assert_eq!(snapshot.read_as::<i32>("k"), 1);
    assert_eq!(snapshot.read_as::<i32>("j"), 2);
}

/// REQ-LOW-lowering-179: a call in the argument of a call with outputs
/// (REQ-LOW-lowering-172) is in place in that argument.
#[test]
fn run_when_result_is_argument_of_call_with_outputs_then_values() {
    let source = format!(
        "{FUNCTIONS}
         FUNCTION SPLIT : DINT
           VAR_INPUT n : DINT; END_VAR VAR_OUTPUT twice : DINT; END_VAR
           SPLIT := n;
           twice := n * 2;
         END_FUNCTION
         PROGRAM main VAR a : STRING[20] := 'ab'; r : DINT; q : DINT; END_VAR
           r := SPLIT(n := LEN(TWICE(a)), twice => q);
         END_PROGRAM"
    );
    assert_same_code(&source);
    let snapshot = run_lowered_agreeing(&source);
    assert_eq!(snapshot.read_as::<i32>("r"), 4);
    assert_eq!(snapshot.read_as::<i32>("q"), 8);
}
