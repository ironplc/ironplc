//! Tests of calls of methods, and of methods' bodies, compiled from the
//! lowered program (REQ-LOW-lowering-076), with every statement required to
//! lower. A program whose methods have only scalar outputs is also checked
//! to leave every variable as the route that reads the AST leaves it after
//! one scan; that route does not read a string output.

use ironplc_codegen::Route;
use rstest::rstest;

use crate::common::{compile_by_route, parse};
use crate::compile_route_lowered::{assert_same_code, options, run_lowered, run_lowered_agreeing};

/// A block whose methods add to its count, read it scaled, and split it
/// into a quotient output, whether that is positive, and a remainder.
const BLOCK: &str = "
    FUNCTION_BLOCK Counter
      VAR n : DINT; END_VAR
      METHOD Add
        VAR_INPUT k : DINT; END_VAR
        n := n + k;
      END_METHOD
      METHOD Get : DINT
        VAR_INPUT scale : DINT; END_VAR
        Get := n * scale;
      END_METHOD
      METHOD Split : DINT
        VAR_INPUT k : DINT; END_VAR
        VAR_OUTPUT high : DINT; done : BOOL; END_VAR
        high := n / k;
        done := high > 0;
        Split := n MOD k;
      END_METHOD
    END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-076: a call runs the method on its receiver's fields,
/// and two instances keep their own. A call whose result is not used drops
/// it; one whose result is used converts it where it is stored.
#[test]
fn run_when_methods_called_then_receivers_fields_change() {
    let snapshot = run_lowered_agreeing(&format!(
        "{BLOCK}
         PROGRAM main
           VAR a : Counter; b : Counter; x : DINT := 4; ra : DINT; rb : LINT; r : DINT; END_VAR
           a.Add(k := x);
           a.Add(k := 3);
           b.Add(k := 10);
           a.Get(scale := 1);
           ra := a.Get(scale := 2);
           rb := b.Get(scale := 3) + 1;
           r := a.Get(scale := 1) + b.Get(scale := 1);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("ra"), 14);
    assert_eq!(snapshot.read_as::<i64>("rb"), 31);
    assert_eq!(snapshot.read_as::<i32>("r"), 17);
}

/// REQ-LOW-lowering-092, REQ-LOW-lowering-172: a method's outputs are
/// stored in their targets after the call, a negated one inverted and one
/// of a narrower type converted, before the result is; a call whose result
/// is not used stores its outputs too.
#[test]
fn run_when_method_with_outputs_called_then_outputs_and_result_stored() {
    let snapshot = run_lowered_agreeing(&format!(
        "{BLOCK}
         PROGRAM main
           VAR c : Counter; high : DINT; low : LINT; done : BOOL := TRUE; wide : LINT;
               r : DINT; END_VAR
           c.Add(k := 17);
           c.Split(k := 5, high => high, done => done);
           r := c.Split(k := 3, high => wide, NOT done => done);
           low := c.Split(k := 4, high => r);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("high"), 3);
    assert_eq!(snapshot.read_as::<i64>("wide"), 5);
    assert_eq!(snapshot.read_as::<i32>("done"), 0);
    assert_eq!(snapshot.read_as::<i64>("low"), 1);
    assert_eq!(snapshot.read_as::<i32>("r"), 4);
}

/// A call in a loop runs at every iteration, and one in a condition and a
/// branch where they run.
#[test]
fn run_when_method_called_in_loop_and_branch_then_each_call() {
    let snapshot = run_lowered_agreeing(&format!(
        "{BLOCK}
         PROGRAM main VAR c : Counter; i : DINT; total : DINT; END_VAR
           FOR i := 1 TO 4 DO c.Add(k := i); END_FOR;
           IF c.Get(scale := 1) > 5 THEN total := c.Get(scale := 10); END_IF;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("total"), 100);
}

/// A call of a method of an instance declared inside a function block stays
/// on the route that reads the AST, which cannot call one, so both routes
/// fail alike.
#[test]
fn compile_when_instance_declared_in_block_then_both_routes_fail() {
    let source = format!(
        "{BLOCK}
         FUNCTION_BLOCK Outer
           VAR inner : Counter; END_VAR
           inner.Add(k := 1);
         END_FUNCTION_BLOCK
         PROGRAM main VAR o : Outer; END_VAR
           o();
         END_PROGRAM"
    );
    let options = options();
    let (library, context) = parse(&source, &options);
    for route in [Route::Ast, Route::PreferLowered] {
        let compiled = compile_by_route(&library, &context, &options, route);
        assert_eq!(
            compiled.err().map(|diagnostic| diagnostic.code),
            Some("P9999".to_string()),
            "{route:?}"
        );
    }
}

/// A call of a method compiles to the same instructions and constants by
/// both routes: the instance pushed, each input, `METHOD_CALL`, the stores
/// of the outputs, then the result dropped or stored.
#[rstest]
#[case::statement("c.Add(k := d);")]
#[case::unused_result("c.Get(scale := 2);")]
#[case::stored_result("d := c.Get(scale := d);")]
#[case::in_expression("d := c.Get(scale := 2) + 1;")]
#[case::outputs_of_statement("c.Split(k := 2, high => d, NOT done => b);")]
#[case::outputs_of_assignment("d := c.Split(k := 3, high => d);")]
#[case::outputs_inside_expression("d := c.Split(k := 3, high => d) + c.Split(k := 2, done => b);")]
#[case::inside_if("IF b THEN c.Add(k := 1); END_IF;")]
fn compile_when_method_called_then_same_code_as_ast(#[case] statement: &str) {
    assert_same_code(&format!(
        "{BLOCK}
         PROGRAM main VAR c : Counter; d : DINT; b : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// REQ-LOW-lowering-092: a method's string output is stored in its target
/// as a string is, at the target's capacity. The route that reads the AST
/// does not read a string output, so the lowered route is the only one that
/// compiles the program.
#[test]
fn run_when_method_has_string_output_then_stored() {
    let snapshot = run_lowered(
        "FUNCTION_BLOCK Named
           VAR count : DINT; END_VAR
           METHOD Label : DINT
             VAR_OUTPUT s : STRING[8]; END_VAR
             count := count + 1;
             s := 'abcdefgh';
             Label := count;
           END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main VAR n : Named; t : STRING[4]; u : STRING; r : DINT; END_VAR
           n.Label(s => u);
           r := n.Label(s => t);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("t"), "abcd");
    assert_eq!(snapshot.read("u"), "abcdefgh");
    assert_eq!(snapshot.read_as::<i32>("r"), 2);
}

/// A call of a method the receiver's block inherits from a block that
/// declares a field stays on the route that reads the AST, which reports it
/// as not implemented (REQ-LOW-codegen-178).
#[test]
fn compile_when_inherited_method_called_then_lowered_route_leaves_it() {
    let source = format!(
        "{BLOCK}
         FUNCTION_BLOCK Derived EXTENDS Counter END_FUNCTION_BLOCK
         PROGRAM main VAR d : Derived; END_VAR
           d.Add(k := 1);
         END_PROGRAM"
    );
    let options = options();
    let (library, context) = parse(&source, &options);
    let lowered = compile_by_route(&library, &context, &options, Route::RequireLowered);
    assert_eq!(
        lowered.err().map(|diagnostic| diagnostic.code),
        Some("P9999".to_string())
    );
}

/// A block with a method of a result and no fields, a block that extends it
/// with a field of its own, and two levels of blocks that extend it with
/// none.
const INHERITED: &str = "
    FUNCTION_BLOCK Base
      METHOD Get : DINT
        VAR_INPUT k : DINT; END_VAR
        Get := k * 10;
      END_METHOD
    END_FUNCTION_BLOCK
    FUNCTION_BLOCK Derived EXTENDS Base VAR n : DINT; END_VAR END_FUNCTION_BLOCK
    FUNCTION_BLOCK Middle EXTENDS Base END_FUNCTION_BLOCK
    FUNCTION_BLOCK Leaf EXTENDS Middle END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-177: a method a block inherits from one that declares no
/// fields runs on an instance of the derived block, and through a block in
/// between.
#[test]
fn run_when_method_inherited_from_base_without_fields_then_base_method_runs() {
    let snapshot = run_lowered(&format!(
        "{INHERITED}
         PROGRAM main VAR d : Derived; l : Leaf; r1 : DINT; r2 : DINT; END_VAR
           r1 := d.Get(k := 2);
           r2 := l.Get(k := 4) + 1;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r1"), 20);
    assert_eq!(snapshot.read_as::<i32>("r2"), 41);
}

/// REQ-LOW-codegen-178: a call of a method a block inherits from one that
/// declares no fields compiles to the same instructions and constants by
/// both routes: the derived instance pushed, each input, then `METHOD_CALL`
/// of the base's method, which copies no field.
#[rstest]
#[case::stored_result("r := d.Get(k := 2);")]
#[case::unused_result("d.Get(k := 3);")]
#[case::through_block_in_between("r := l.Get(k := 4) + 1;")]
fn compile_when_method_inherited_from_base_without_fields_then_same_code_as_ast(
    #[case] statement: &str,
) {
    assert_same_code(&format!(
        "{INHERITED}
         PROGRAM main VAR d : Derived; l : Leaf; r : DINT; END_VAR
           {statement}
         END_PROGRAM"
    ));
}
