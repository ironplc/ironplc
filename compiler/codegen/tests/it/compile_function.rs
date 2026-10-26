//! Tests of calls of user-defined functions compiled from the lowered
//! program.

use rstest::rstest;

use crate::compile_body::{assert_compiles, compile_and_run, compile_and_run_agreeing};

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
    let snapshot = compile_and_run(&format!(
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

/// A call of a user-defined function lowers and compiles.
#[rstest]
#[case::positional("d := ADD3(d, e, 1);")]
#[case::named_out_of_order("d := ADD3(c := d, a := e, b := 1);")]
#[case::converted_argument("d := ADD3(s, d, s);")]
#[case::narrow_result("i := HALF(i) + HALF(s);")]
#[case::converted_result("i := SCALE(1.5, 2);")]
#[case::nested("d := ADD3(ADD3(d, e, 1), TWICE(e), COUNT()) + 1;")]
fn compile_when_user_function_call_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main
           VAR d : DINT; e : DINT; s : SINT; i : INT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
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
    let snapshot = compile_and_run(&format!(
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
/// variable through its reference, leaving the other bits as they were.
#[test]
fn run_when_in_out_has_bits_written_then_callers_variable() {
    let snapshot = compile_and_run(
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

/// A call of a function with a `VAR_IN_OUT`, and the function's body, lower
/// and compile: the call passes a variable's index, or the reference a
/// `VAR_IN_OUT` of the caller holds, and the body loads and stores through
/// it.
#[rstest]
#[case::variable("d := INC(v := d, n := e);")]
#[case::passed_on("d := INC_TWICE(e);")]
#[case::two_in_outs("x := SWAP(i, k);")]
fn compile_when_in_out_call_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "{IN_OUT_FUNCTIONS}
         PROGRAM main
           VAR d : DINT; e : DINT; i : DINT; k : DINT; x : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// REQ-LOW-lowering-075: a structure or array input of a function or method
/// is passed as the offset of the argument's region, so each call compiles
/// and leaves every variable with the same values on every backend. This
/// backend compiles no read of such an input in the callee's body.
#[test]
fn run_when_aggregate_input_passed_then_compiles() {
    let source = "
        TYPE P : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
        TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE
        FUNCTION SUMP : DINT VAR_INPUT q : P; n : DINT; END_VAR SUMP := n; END_FUNCTION
        FUNCTION SUMA : DINT
          VAR_INPUT q : A3; END_VAR VAR_OUTPUT o : DINT; END_VAR
          SUMA := 8;
          o := 1;
        END_FUNCTION
        FUNCTION_BLOCK BP METHOD M : DINT VAR_INPUT q : P; END_VAR M := 5; END_METHOD
        END_FUNCTION_BLOCK
        PROGRAM main
          VAR pt : P := (x := 1, y := 2); a3 : A3; f : BP; r1 : DINT; r2 : DINT; r3 : DINT;
              k : DINT; END_VAR
          r1 := SUMP(q := pt, n := 7);
          r2 := SUMA(q := a3, o => k) + 1;
          r3 := f.M(q := pt);
        END_PROGRAM";
    assert_compiles(source);
    let snapshot = compile_and_run_agreeing(source);
    assert_eq!(snapshot.read_as::<i32>("r1"), 7);
    assert_eq!(snapshot.read_as::<i32>("r2"), 9);
    assert_eq!(snapshot.read_as::<i32>("k"), 1);
    assert_eq!(snapshot.read_as::<i32>("r3"), 5);
}

/// Blocks with methods whose results are a structure and an array, which a
/// call can only discard.
const AGGREGATE_RESULTS: &str = "
    TYPE P : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
    TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE
    FUNCTION_BLOCK BP VAR n : DINT; END_VAR VAR_OUTPUT o : DINT; END_VAR
      METHOD MKP : P VAR_INPUT v : DINT; END_VAR n := n + v; END_METHOD
      METHOD MKA : A3 VAR_INPUT v : DINT; END_VAR VAR_OUTPUT k : DINT; END_VAR
        n := n * v; k := n;
      END_METHOD
    END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-187: a call of a method whose result is a structure or
/// an array, which a statement discards, lowers and compiles, with an output
/// stored after it.
#[rstest]
#[case::structure_result("b.MKP(v := 3);")]
#[case::array_result_with_output("b.MKA(v := 2, k => r);")]
fn compile_when_method_aggregate_result_discarded_then_compiles(#[case] statement: &str) {
    let source = format!(
        "{AGGREGATE_RESULTS}
         PROGRAM main VAR b : BP; r : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// REQ-LOW-lowering-187: calls of methods whose results are a structure and
/// an array leave every variable with the same values on every backend.
#[test]
fn run_when_method_aggregate_result_discarded_then_backends_agree() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{AGGREGATE_RESULTS}
         PROGRAM main VAR b : BP; r : DINT; END_VAR
           b.MKP(v := 3);
           b.MKA(v := 2, k => r);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r"), 6);
}

/// Functions with a `VAR_IN_OUT` of a `DINT` and of a type declared as
/// `DINT`.
const IN_OUT_ALIASES: &str = "
    TYPE MyInt : DINT; END_TYPE
    FUNCTION F : DINT VAR_IN_OUT x : DINT; END_VAR x := x + 1; F := x; END_FUNCTION
    FUNCTION G : DINT VAR_IN_OUT x : MyInt; END_VAR x := x * 2; G := x; END_FUNCTION";

/// REQ-LOW-lowering-072: a variable of a type declared as `DINT` passed for
/// a `VAR_IN_OUT` of a `DINT`, and the reverse, lower and compile.
#[rstest]
#[case::argument_of_alias_type("r := F(x := a);")]
#[case::parameter_of_alias_type("r := G(x := d);")]
fn compile_when_in_out_argument_type_is_alias_then_compiles(#[case] statement: &str) {
    let source = format!(
        "{IN_OUT_ALIASES}
         PROGRAM main VAR a : MyInt := 4; d : DINT := 5; r : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// REQ-LOW-lowering-072: the callee writes the caller's variable through a
/// `VAR_IN_OUT` whose type is an alias of the variable's, or the reverse.
#[test]
fn run_when_in_out_argument_type_is_alias_then_callee_writes_it() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{IN_OUT_ALIASES}
         PROGRAM main VAR a : MyInt := 4; d : DINT := 5; r1 : DINT; r2 : DINT; END_VAR
           r1 := F(x := a);
           r2 := G(x := d);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("a"), 5);
    assert_eq!(snapshot.read_as::<i32>("r1"), 5);
    assert_eq!(snapshot.read_as::<i32>("d"), 10);
    assert_eq!(snapshot.read_as::<i32>("r2"), 10);
}

/// Blocks and a function whose inputs are an instance of a block, a standard
/// block and a structure, none of which their bodies call or read.
const INSTANCE_INPUTS: &str = "
    TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
    FUNCTION_BLOCK INNER VAR_INPUT v : DINT; END_VAR VAR_OUTPUT q : DINT; END_VAR q := v; END_FUNCTION_BLOCK
    FUNCTION_BLOCK OUTER VAR_INPUT inner : INNER; p : P; END_VAR END_FUNCTION_BLOCK
    FUNCTION SUMP : DINT VAR_INPUT q : P; END_VAR SUMP := 6; END_FUNCTION
    FUNCTION_BLOCK BP
      METHOD Go : DINT VAR_INPUT i : INNER; d : DINT; END_VAR Go := d * 2; END_METHOD
      METHOD Tm : DINT VAR_INPUT t : TON; END_VAR Tm := 3; END_METHOD
    END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-075: a method's input of a function block type, of a
/// user-defined or a standard block, and an input taken from a field of
/// another instance, lower and compile: each is passed as the slot of the
/// instance or of the field, which holds the data offset of the instance's
/// fields or of the field's value.
#[rstest]
#[case::instance("r := b.Go(i := f, d := 5);")]
#[case::standard_instance("r := b.Tm(t := t);")]
#[case::field_of_instance("r := b.Go(i := g.inner, d := 1);")]
#[case::structure_field_of_instance("r := SUMP(q := g.p);")]
fn compile_when_instance_or_field_input_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "{INSTANCE_INPUTS}
         PROGRAM main
           VAR f : INNER; g : OUTER; b : BP; t : TON; r : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// Instances and fields of instances passed as inputs leave every variable
/// with the same value on every backend: the instance keeps its own
/// fields, and the methods and the function return what their scalar inputs
/// give. This backend compiles no call or read of such an input in the
/// callee's body (issue 1553).
#[test]
fn run_when_instance_or_field_input_passed_then_backends_agree() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{INSTANCE_INPUTS}
         PROGRAM main
           VAR f : INNER; g : OUTER; b : BP; t : TON;
               r1 : DINT; r2 : DINT; r3 : DINT; r4 : DINT; q : DINT; END_VAR
           f(v := 4);
           r1 := b.Go(i := f, d := 5);
           r2 := b.Tm(t := t);
           r3 := b.Go(i := g.inner, d := 2);
           r4 := SUMP(q := g.p);
           q := f.q;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r1"), 10);
    assert_eq!(snapshot.read_as::<i32>("r2"), 3);
    assert_eq!(snapshot.read_as::<i32>("r3"), 4);
    assert_eq!(snapshot.read_as::<i32>("r4"), 6);
    assert_eq!(snapshot.read_as::<i32>("q"), 4);
}
