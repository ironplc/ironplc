//! Tests of control flow compiled from the lowered program.
//!
//! The end-to-end tests compare both routes on every program, but only for
//! the statements that happen to lower. These require the lowered route, so
//! each program here is known to take it: the edge cases run and are checked
//! by value, and the shapes are checked to compile to the same instructions
//! as the AST route.

use ironplc_codegen::Route;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{bc, compile_by_route, parse, Snapshot};

fn options() -> CompilerOptions {
    CompilerOptions {
        allow_continue: true,
        ..CompilerOptions::default()
    }
}

/// Compiles `source` with every statement lowered and runs it for one scan.
/// The scan fails the test if it leaves a value on the operand stack.
fn run_lowered(source: &str) -> Snapshot {
    let options = options();
    let (library, context) = parse(source, &options);
    let container = compile_by_route(&library, &context, &options, Route::RequireLowered)
        .expect("every statement lowers");
    Snapshot::run_compiled(container)
}

/// An `EXIT` inside a `CASE` arm leaves the loop with the selector already
/// popped.
#[test]
fn run_when_exit_in_case_arm_then_leaves_loop_with_stack_balanced() {
    let snapshot = run_lowered(
        "PROGRAM main VAR i : DINT; hits : DINT; END_VAR
           FOR i := 1 TO 10 DO
             CASE i OF
               3: EXIT;
             ELSE
               hits := hits + 1;
             END_CASE;
           END_FOR;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("i"), 3);
    assert_eq!(snapshot.read_as::<i64>("hits"), 2);
}

/// A `RETURN` inside a `CASE` arm returns with the selector already popped.
#[test]
fn run_when_return_in_case_arm_then_returns_with_stack_balanced() {
    let snapshot = run_lowered(
        "PROGRAM main VAR x : DINT := 1; a : DINT; b : DINT; END_VAR
           CASE x OF
             1: a := 10; RETURN;
           END_CASE;
           b := 20;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("a"), 10);
    assert_eq!(snapshot.read_as::<i64>("b"), 0);
}

/// Unsigned and 64-bit selectors compare by their own signedness and width,
/// at the extremes of their types.
#[test]
fn run_when_case_selector_unsigned_or_64_bit_then_matches_by_its_type() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR u : UDINT := 4294967295; l : LINT := -9223372036854775808;
               ul : ULINT := 18446744073709551615; a : DINT; b : DINT; c : DINT; END_VAR
           CASE u OF
             0..10: a := 1;
             3000000000..4294967295: a := 2;
           END_CASE;
           CASE l OF
             -9223372036854775808..-1: b := 1;
           ELSE
             b := 2;
           END_CASE;
           CASE ul OF
             0..10: c := 1;
             18446744073709551615: c := 2;
           END_CASE;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("a"), 2);
    assert_eq!(snapshot.read_as::<i64>("b"), 1);
    assert_eq!(snapshot.read_as::<i64>("c"), 2);
}

/// A `FOR` over a `LINT` counts at 64 bits, with a step the analyzer
/// converted to the control's type.
#[test]
fn run_when_for_over_lint_with_converted_step_then_counts() {
    let snapshot = run_lowered(
        "PROGRAM main VAR l : LINT; n : DINT; END_VAR
           FOR l := 0 TO 10 BY DINT#3 DO n := n + 1; END_FOR;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("n"), 4);
    assert_eq!(snapshot.read_as::<i64>("l"), 12);
}

/// A `FOR` counting an `INT` down from a variable keeps the truncation of
/// its control, since no constant bound proves it unneeded.
#[test]
fn run_when_for_counts_int_down_from_variable_then_counts() {
    let snapshot = run_lowered(
        "PROGRAM main VAR start : INT := 10; k : INT; n : DINT; END_VAR
           FOR k := start TO 1 BY -3 DO n := n + 1; END_FOR;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("n"), 4);
    assert_eq!(snapshot.read_as::<i64>("k"), -2);
}

/// `CONTINUE` goes on with the next iteration of each kind of loop: to the
/// step of a `FOR`, the condition of a `WHILE`, and the `UNTIL` of a
/// `REPEAT`.
#[test]
fn run_when_continue_in_each_loop_then_next_iteration() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR i : DINT; f : DINT; w : DINT; ws : DINT; r : DINT; rs : DINT; END_VAR
           FOR i := 1 TO 5 DO
             IF i = 2 THEN CONTINUE; END_IF;
             f := f + i;
           END_FOR;
           WHILE w < 5 DO
             w := w + 1;
             IF w = 3 THEN CONTINUE; END_IF;
             ws := ws + w;
           END_WHILE;
           REPEAT
             r := r + 1;
             IF r = 2 THEN CONTINUE; END_IF;
             rs := rs + r;
           UNTIL r >= 4 END_REPEAT;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("f"), 13);
    assert_eq!(snapshot.read_as::<i64>("ws"), 12);
    assert_eq!(snapshot.read_as::<i64>("rs"), 8);
}

/// Compiles `source` by `route`, and returns the bytecode of its program body.
fn body_by_route(source: &str, route: Route) -> Vec<u8> {
    let options = options();
    let (library, context) = parse(source, &options);
    let container = compile_by_route(&library, &context, &options, route).unwrap();
    container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap()
        .to_vec()
}

/// REQ-LOW-lowering-046: a `FOR` whose first value is a read of a variable
/// of the control's type stores it untruncated, since it already fits. The
/// AST route truncates it, a no-op; the rest of the loop is the same.
#[test]
fn compile_when_for_first_value_fits_control_then_lowered_route_drops_its_truncation() {
    let source = "PROGRAM main VAR k : INT; s : INT; n : DINT; END_VAR
           FOR k := s TO 10 DO n := n + 1; END_FOR;
         END_PROGRAM";
    let ast = body_by_route(source, Route::Ast);
    let lowered = body_by_route(source, Route::RequireLowered);
    // k is var:0, s var:1.
    let start = [bc::load_var_i32(1), bc::store_var_i32(0)].concat();
    let truncated_start = [bc::load_var_i32(1), bc::trunc_i16(), bc::store_var_i32(0)].concat();
    assert!(lowered.starts_with(&start), "{lowered:?}");
    assert!(ast.starts_with(&truncated_start), "{ast:?}");
    assert_eq!(lowered[start.len()..], ast[truncated_start.len()..]);
}

/// Each control-flow statement compiles to the same instructions and
/// constants by both routes, fused compare and branch included. `CASE` is
/// the exception, since the lowered route evaluates its selector once
/// (`compile_route.rs`).
#[rstest]
#[case::while_fused("WHILE w < 5 DO w := w + 1; END_WHILE;")]
#[case::while_unfused("WHILE w < n DO w := w + 1; END_WHILE;")]
#[case::repeat_fused("REPEAT w := w + 1; UNTIL w > 5 END_REPEAT;")]
#[case::repeat_unfused("REPEAT w := w + 1; UNTIL w > n END_REPEAT;")]
#[case::for_fused("FOR w := 1 TO 10 DO n := n + w; END_FOR;")]
#[case::for_unfused("FOR w := 1 TO n DO n := n - 1; END_FOR;")]
#[case::for_down("FOR w := 10 TO 1 BY -2 DO n := n + w; END_FOR;")]
#[case::for_narrow_in_range("FOR k := 1 TO 10 DO n := n + 1; END_FOR;")]
#[case::if_fused("IF w > 3 THEN n := 1; END_IF;")]
#[case::if_elsif_else("IF w > 3 THEN n := 1; ELSIF w < n THEN n := 2; ELSE n := 3; END_IF;")]
#[case::exit_and_continue(
    "FOR w := 1 TO 10 DO IF w = 2 THEN CONTINUE; END_IF; IF w > 5 THEN EXIT; END_IF; END_FOR;"
)]
#[case::narrow_store("WHILE k < 5 DO k := k + 1; END_WHILE;")]
fn compile_when_control_flow_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main VAR w : DINT; n : DINT; k : INT; s : INT; END_VAR
           {statement}
         END_PROGRAM"
    );
    let options = options();
    let (library, context) = parse(&source, &options);
    let ast = compile_by_route(&library, &context, &options, Route::Ast).unwrap();
    let lowered = compile_by_route(&library, &context, &options, Route::RequireLowered)
        .expect("every statement lowers");
    assert_eq!(format!("{:?}", lowered.code), format!("{:?}", ast.code));
    assert_eq!(
        format!("{:?}", lowered.constant_pool),
        format!("{:?}", ast.constant_pool)
    );
}
