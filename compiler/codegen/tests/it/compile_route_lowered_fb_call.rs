//! Tests of calls of user-defined function blocks compiled from the lowered
//! program (REQ-LOW-lowering-069), with every statement required to lower,
//! and one scan of each program checked to leave every variable as the
//! route that reads the AST leaves it.

use ironplc_codegen::Route;
use ironplc_parser::options::CompilerOptions;

use rstest::rstest;

use crate::common::{compile_by_route, parse, parse_and_compile_to_run, Snapshot};
use crate::compile_route_lowered::{assert_same_code, run_lowered};

/// A block whose outputs are its inputs' sum, whether the sum is positive,
/// a `REAL`, a `UDINT` and a count of its calls.
const BLOCK: &str = "
    FUNCTION_BLOCK FB
      VAR_INPUT x : DINT; s : SINT; u : UDINT; END_VAR
      VAR_OUTPUT q : DINT; done : BOOL; r : REAL; qu : UDINT; calls : DINT; END_VAR
      q := x + s;
      done := q > 0;
      r := 1.5;
      qu := u;
      calls := calls + 1;
    END_FUNCTION_BLOCK";

/// Runs `source` with every statement lowered, once one scan of it by each
/// route is shown to leave every variable with the same value.
fn run_lowered_agreeing(source: &str) -> Snapshot {
    parse_and_compile_to_run(source, &CompilerOptions::default());
    run_lowered(source)
}

/// REQ-LOW-lowering-069: a call stores its inputs, in the order the block
/// declares them, runs the block, and stores each output in its target: a
/// negated one inverted, one of a narrower type converted to its target's,
/// and one in an element of an array at a literal subscript.
#[test]
fn run_when_block_called_then_inputs_stored_and_outputs_read() {
    let snapshot = run_lowered_agreeing(&format!(
        "{BLOCK}
         PROGRAM main
           VAR f : FB; a : DINT := 40; b : BOOL := TRUE; lr : LREAL; lu : LINT;
               arr : ARRAY[1..2] OF DINT; element : DINT; END_VAR
           f(u := 4000000000, s := 2, x := a, q => a, NOT done => b, r => lr, qu => lu,
             calls => arr[2]);
           element := arr[2];
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("a"), 42);
    assert_eq!(snapshot.read_as::<i32>("b"), 0);
    assert_eq!(snapshot.read_as::<f64>("lr"), 1.5);
    assert_eq!(snapshot.read_as::<i64>("lu"), 4_000_000_000);
    assert_eq!(snapshot.read_as::<i32>("element"), 1);
}

/// REQ-LOW-lowering-069: an input a call omits keeps the value the previous
/// call stored, and the instance keeps its fields from one call to the next.
#[test]
fn run_when_block_called_twice_then_omitted_input_and_fields_kept() {
    let snapshot = run_lowered_agreeing(&format!(
        "{BLOCK}
         PROGRAM main VAR f : FB; first : DINT; second : DINT; n : DINT; END_VAR
           f(x := 5, s := 1, q => first);
           f(s := -3, q => second, calls => n);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("first"), 6);
    assert_eq!(snapshot.read_as::<i32>("second"), 2);
    assert_eq!(snapshot.read_as::<i32>("n"), 2);
}

/// A call inside an `IF` and a `FOR` runs each time its statement does, and
/// two instances of one block keep separate fields.
#[test]
fn run_when_block_called_in_loop_and_if_then_runs_each_time() {
    let snapshot = run_lowered_agreeing(&format!(
        "{BLOCK}
         PROGRAM main
           VAR f : FB; g : FB; i : DINT; total : DINT; skipped : DINT; fc : DINT; gc : DINT;
           END_VAR
           FOR i := 1 TO 4 DO
             f(x := i, q => total);
           END_FOR;
           IF total > 100 THEN
             g(x := 1, calls => skipped);
           END_IF;
           fc := f.calls;
           gc := g.calls;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("total"), 4);
    assert_eq!(snapshot.read_as::<i32>("fc"), 4);
    assert_eq!(snapshot.read_as::<i32>("gc"), 0);
    assert_eq!(snapshot.read_as::<i32>("skipped"), 0);
}

/// REQ-LOW-lowering-046: an input is stored in its field as `f.s := v + 100`
/// stores it, narrowed to the field's storage, by either route (#2292).
#[test]
fn run_when_input_outside_narrow_field_then_narrowed() {
    let snapshot = run_lowered_agreeing(
        "FUNCTION_BLOCK Narrow
           VAR_INPUT s : SINT; u : USINT; END_VAR
           VAR_OUTPUT q : DINT; qu : DINT; END_VAR
           q := s;
           qu := u;
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR f : Narrow; v : SINT := 100; w : USINT := 200; r : DINT; ru : DINT; END_VAR
           f(s := v + 100, u := w + 100, q => r, qu => ru);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("r"), -56);
    assert_eq!(snapshot.read_as::<i32>("ru"), 44);
}

/// A call compiles to the same instructions and constants by both routes:
/// the instance pushed once, each input narrowed and stored in its field,
/// the call, then each output stored. A call inside an `IF` too.
#[rstest]
#[case::inputs_and_outputs("f(s := 2, x := d, q => d, NOT done => b, r => lr);")]
#[case::narrowed_input("f(s := n + 1);")]
#[case::no_arguments("f();")]
#[case::outputs_only("f(q => d, qu => lu);")]
#[case::inside_if("IF b THEN f(x := d, q => d); END_IF;")]
#[case::two_calls_in_a_loop("FOR d := 1 TO 3 DO f(x := d); g(x := d, q => n2); END_FOR;")]
fn compile_when_block_called_then_same_code_as_ast(#[case] statement: &str) {
    assert_same_code(&format!(
        "{BLOCK}
         PROGRAM main
           VAR f : FB; g : FB; d : DINT; n : SINT; n2 : DINT; b : BOOL; lr : LREAL; lu : LINT;
           END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// A call of an instance declared inside a function block stays on the
/// route that reads the AST, which cannot call one (#1553), so both routes
/// fail alike.
#[test]
fn compile_when_instance_declared_in_block_then_lowered_route_leaves_it() {
    let source = format!(
        "{BLOCK}
         FUNCTION_BLOCK Outer
           VAR inner : FB; n : DINT; END_VAR
           inner(x := n);
         END_FUNCTION_BLOCK
         PROGRAM main VAR o : Outer; END_VAR
           o();
         END_PROGRAM"
    );
    let options = CompilerOptions::default();
    let (library, context) = parse(&source, &options);
    let ast = compile_by_route(&library, &context, &options, Route::Ast);
    let lowered = compile_by_route(&library, &context, &options, Route::PreferLowered);
    assert_eq!(
        ast.err().map(|diagnostic| diagnostic.code),
        lowered.err().map(|diagnostic| diagnostic.code)
    );
}
