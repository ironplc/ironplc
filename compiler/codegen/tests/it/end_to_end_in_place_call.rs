//! End-to-end tests of in-place calls: calls of functions and methods with
//! outputs, and of functions whose result is a string, run where the
//! source evaluates them (REQ-LOW-lowering-172, REQ-LOW-lowering-174,
//! REQ-LOW-lowering-179, REQ-LOW-lowering-180).
//!
//! Each program compiles (REQ-LOW-codegen-175) and leaves every variable
//! with the same value on every backend after one scan.

use ironplc_vm::error::Trap;
use rstest::rstest;

use crate::common::{compile_analyzed, parse, parse_and_compile_to_run, run_one_scan, Snapshot};
use crate::compile_body::{assert_compiles, compile_and_run_agreeing, options};

/// The functions, blocks and variables the programs share.
///
/// `F` gives `x * 10` and outputs `x + 1`; `G` doubles its string; `GO`
/// gives `'ab'` and outputs its input; `B.M` gives `d * 2` and outputs `d`.
const DECLARATIONS: &str = "
TYPE P : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
FUNCTION F : DINT
  VAR_INPUT x : DINT; END_VAR VAR_OUTPUT n : DINT; END_VAR
  n := x + 1; F := x * 10;
END_FUNCTION
FUNCTION G : STRING
  VAR_INPUT s : STRING; END_VAR
  G := CONCAT(s, s);
END_FUNCTION
FUNCTION GO : STRING
  VAR_INPUT x : DINT; END_VAR VAR_OUTPUT n : DINT; END_VAR
  n := x; GO := 'ab';
END_FUNCTION
FUNCTION ONE : DINT
  ONE := 1;
END_FUNCTION
FUNCTION FW : DINT
  VAR_INPUT x : DINT; END_VAR VAR_OUTPUT q : WORD; END_VAR
  q := WORD#2; FW := x;
END_FUNCTION
FUNCTION FI : INT
  VAR_INPUT x : DINT; END_VAR VAR_OUTPUT n : DINT; END_VAR
  n := x + 1; FI := 7;
END_FUNCTION
FUNCTION W : DINT
  VAR_INPUT d : DINT; END_VAR VAR k : DINT; END_VAR
  W := 1 + F(x := d, n => k);
END_FUNCTION
FUNCTION_BLOCK B
  METHOD M : DINT
    VAR_INPUT d : DINT; END_VAR VAR_OUTPUT n : DINT; END_VAR
    n := d; M := d * 2;
  END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FBV
  VAR_INPUT v : DINT; END_VAR VAR_OUTPUT q : DINT; END_VAR
  q := v;
END_FUNCTION_BLOCK";

/// `statements` in a program with the declarations' variables.
fn program(statements: &str) -> String {
    format!(
        "{DECLARATIONS}
         PROGRAM main
           VAR k, j, n, r, i : DINT; a : DINT := 2; b : BOOL; c : BOOL := TRUE;
               s : STRING := 'a'; t : STRING := 'b'; w : WORD; arr : ARRAY[0..30] OF DINT;
               f : B; fb : FBV; ri : INT; rd : LREAL; END_VAR
           {statements}
         END_PROGRAM"
    )
}

/// Each program lowers, compiles, and leaves its integers with the values
/// given.
#[rstest]
// A string result after a read, a call or an operation in its statement.
#[case::string_after_read("n := k + LEN(G(s));", &[("n", 2)])]
#[case::string_compared("b := s = G(s);", &[("b", 0)])]
#[case::string_after_call("n := ONE() + LEN(G(s));", &[("n", 3)])]
#[case::two_strings("n := LEN(G(s)) + LEN(G(t));", &[("n", 4)])]
#[case::string_after_output("n := F(x := 1, n => k) + LEN(G(s));", &[("n", 12), ("k", 2)])]
// A string result in a `FOR`'s `TO`, a bit write and the right of
// `AND_THEN` and `OR_ELSE`.
#[case::string_in_for_to("FOR i := 1 TO LEN(G(s)) DO k := k + 1; END_FOR;", &[("k", 2), ("i", 3)])]
#[case::string_in_bit_write("w.3 := LEN(G(s)) > 0;", &[("w", 8)])]
#[case::string_and_then("b := c AND_THEN G(s) = 'aa';", &[("b", 1)])]
#[case::string_or_else("b := c OR_ELSE G(s) = 'aa';", &[("b", 1)])]
#[case::string_with_output("n := LEN(GO(x := 1, n => k));", &[("n", 2), ("k", 1)])]
// A call with outputs after a read, a constant or an operation.
#[case::output_after_read("r := a + F(x := 1, n => a);", &[("r", 12), ("a", 2)])]
#[case::output_after_constant("r := 1 + F(x := 1, n => k);", &[("r", 11), ("k", 2)])]
#[case::outputs_in_operations(
    "r := F(x := 1, n => k) * 2 + F(x := 2, n => j);",
    &[("r", 40), ("k", 2), ("j", 3)]
)]
#[case::output_and_then("b := c AND_THEN F(x := 1, n => k) > 0;", &[("b", 1), ("k", 2)])]
// A call with outputs in a subscript, a condition and a `FOR`'s bounds.
#[case::output_in_read_subscript(
    "arr[10] := 7; r := arr[F(x := 1, n => k)];",
    &[("r", 7), ("k", 2)]
)]
#[case::output_in_written_subscript(
    "arr[F(x := 1, n => k)] := 5; r := arr[10];",
    &[("r", 5), ("k", 2)]
)]
#[case::output_in_if("IF F(x := 1, n => k) > 0 THEN r := 1; END_IF;", &[("r", 1), ("k", 2)])]
#[case::output_in_while(
    "WHILE F(x := i, n => k) < 30 DO i := i + 1; END_WHILE;",
    &[("i", 3), ("k", 4)]
)]
#[case::output_in_for_to(
    "FOR i := 1 TO F(x := 1, n => k) DO r := r + 1; END_FOR;",
    &[("r", 10), ("i", 11), ("k", 2)]
)]
#[case::output_in_for_from(
    "FOR i := F(x := 1, n => k) TO 20 DO r := r + 1; END_FOR;",
    &[("r", 11), ("i", 21), ("k", 2)]
)]
#[case::output_in_bit_write("w.3 := F(x := 1, n => k) > 0;", &[("w", 8), ("k", 2)])]
// A call with outputs in a function block call's input, and of a method.
#[case::output_in_block_input("fb(v := F(x := 1, n => k), q => r);", &[("r", 10), ("k", 2)])]
#[case::method_output_after_constant("r := 1 + f.M(d := 1, n => k);", &[("r", 3), ("k", 1)])]
#[case::method_output_in_if(
    "IF f.M(d := 1, n => k) > 0 THEN r := 1; END_IF;",
    &[("r", 1), ("k", 1)]
)]
// REQ-LOW-lowering-174: `a` is read before the call stores its output in
// it, so the sum is of the old `a`.
#[case::output_stored_after_earlier_read("r := a + F(x := 5, n => a);", &[("r", 52), ("a", 6)])]
// A result narrower than its operation type is narrowed where it is
// stored, after the outputs, and converted without narrowing.
#[case::narrow_result_with_outputs(
    "ri := FI(x := 1, n => k); r := FI(x := 2, n => j);",
    &[("ri", 7), ("r", 7), ("k", 2), ("j", 3)]
)]
fn run_when_call_in_place_then_same_code_and_values(
    #[case] statements: &str,
    #[case] expected: &[(&str, i64)],
) {
    let source = program(statements);
    assert_compiles(&source);
    let snapshot = compile_and_run_agreeing(&source);
    let actual: Vec<(&str, i64)> = expected
        .iter()
        .map(|(name, _)| (*name, snapshot.read_as::<i64>(name)))
        .collect();
    assert_eq!(actual, expected);
}

/// A string result assigned whole or in a loop's condition lowers, compiles,
/// and leaves `s` with the string given and `k` with the value given.
#[rstest]
#[case::string_operand_after_read("s := CONCAT(t, G(s));", "baa", 0)]
#[case::string_in_while("WHILE LEN(G(s)) < 8 DO s := CONCAT(s, 'x'); END_WHILE;", "axxx", 0)]
#[case::string_in_repeat(
    "REPEAT s := CONCAT(s, 'x'); UNTIL LEN(G(s)) > 8 END_REPEAT;",
    "axxxx",
    0
)]
#[case::string_result_with_output("s := GO(x := 1, n => k);", "ab", 1)]
fn run_when_string_result_in_place_then_same_code_and_values(
    #[case] statements: &str,
    #[case] text: &str,
    #[case] k: i64,
) {
    let source = program(statements);
    assert_compiles(&source);
    let snapshot = compile_and_run_agreeing(&source);
    assert_eq!(snapshot.read("s"), text);
    assert_eq!(snapshot.read_as::<i64>("k"), k);
}

/// A call in place leaves every variable with the same values on every
/// backend:
///
/// * in a `CASE` selector, which is evaluated once (REQ-LOW-codegen-124);
/// * in an `ELSIF` condition with no `ELSE`;
/// * storing an output in a `WORD` whose bit the statement writes.
///   `AssignBits` reads `w` before its value, so the output the call stores
///   in `w` is lost (REQ-LOW-codegen-185).
#[rstest]
#[case::string_in_selector(
    "CASE LEN(G(s)) OF 2: k := 1; ELSE k := 2; END_CASE;",
    &[("k", 1)]
)]
#[case::output_in_selector(
    "CASE F(x := 1, n => k) OF 10: r := 1; ELSE r := 2; END_CASE;",
    &[("r", 1), ("k", 2)]
)]
#[case::string_in_elsif(
    "IF a > 5 THEN k := 1; ELSIF G(s) = 'aa' THEN k := 2; END_IF;",
    &[("k", 2)]
)]
#[case::output_to_bit_written_place_lost("w.3 := FW(x := 1, q => w) > 0;", &[("w", 8)])]
fn run_when_call_in_place_then_backends_agree(
    #[case] statements: &str,
    #[case] expected: &[(&str, i64)],
) {
    let snapshot = compile_and_run_agreeing(&program(statements));
    let actual: Vec<(&str, i64)> = expected
        .iter()
        .map(|(name, _)| (*name, snapshot.read_as::<i64>(name)))
        .collect();
    assert_eq!(actual, expected);
}

/// A call with outputs in a function's body lowers. Inside a function's
/// body the VM reaches no variable of another function, so reading the
/// output traps (`InvalidVariableIndex`).
#[test]
fn run_when_call_with_outputs_in_function_body_then_traps() {
    let source = program("r := W(d := 3);");
    let options = options();
    parse_and_compile_to_run(&source, &options);
    let (library, context) = parse(&source, &options);
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    let Err(fault) = run_one_scan(&container) else {
        panic!("reading another function's output traps");
    };
    assert!(matches!(fault.trap, Trap::InvalidVariableIndex(_)));
}

/// REQ-LOW-codegen-124: a call with outputs in a `CASE` selector runs once,
/// whatever label matches, so it counts one call here and does not run the
/// `ELSE`.
#[test]
fn run_when_call_with_outputs_in_case_selector_then_called_once() {
    let source = "
        FUNCTION COUNTED : DINT
          VAR_IN_OUT calls : DINT; END_VAR VAR_OUTPUT seen : DINT; END_VAR
          calls := calls + 1; seen := calls; COUNTED := calls;
        END_FUNCTION
        PROGRAM main VAR calls : DINT; seen : DINT; r : DINT; END_VAR
          CASE COUNTED(calls := calls, seen => seen) OF
            5: r := 5;
            6: r := 6;
            1: r := 1;
          ELSE
            r := 0;
          END_CASE;
        END_PROGRAM";
    let options = options();
    let (library, context) = parse(source, &options);
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    let snapshot = Snapshot::run_compiled(container);
    assert_eq!(snapshot.read_as::<i64>("calls"), 1);
    assert_eq!(snapshot.read_as::<i64>("seen"), 1);
    assert_eq!(snapshot.read_as::<i64>("r"), 1);
}
