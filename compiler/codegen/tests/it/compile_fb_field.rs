//! Tests of the fields of function block instances read and written from the
//! lowered program.

use rstest::rstest;

use crate::common::Duration;
use crate::compile_body::{assert_compiles, compile_and_run, compile_and_run_agreeing};

/// A block with fields of several types, which no test calls.
const BLOCK: &str = "
    FUNCTION_BLOCK B
      VAR_INPUT a : DINT; w : WORD; t : TIME; END_VAR
      VAR_OUTPUT q : BOOL; END_VAR
      q := a > 0;
    END_FUNCTION_BLOCK";

/// A field of a user-defined and of a standard block is read and written
/// outside a call, a member initializer sets one, and a bit of a field is
/// written and read.
#[test]
fn run_when_block_fields_read_and_written_then_values() {
    let snapshot = compile_and_run(&format!(
        "{BLOCK}
         PROGRAM main
           VAR b : B := (t := T#250MS); c : CTU; s : TON := (PT := T#2S);
               r1 : DINT; r2 : BOOL; r3 : TIME; r4 : WORD; r5 : BOOL; r6 : INT; r7 : TIME;
               r8 : BOOL; END_VAR
           b.a := 41;
           b.a := b.a + 1;
           r1 := b.a;
           r2 := b.q;
           r3 := b.t;
           b.w.3 := TRUE;
           r4 := b.w;
           r5 := b.w.3;
           c.PV := 7;
           r6 := c.PV;
           r7 := s.PT;
           r8 := c.Q;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r1"), 42);
    assert_eq!(snapshot.read_as::<u64>("r2"), 0);
    assert_eq!(snapshot.read("r3"), Duration::milliseconds(250));
    assert_eq!(snapshot.read_as::<u64>("r4"), 8);
    assert_eq!(snapshot.read_as::<u64>("r5"), 1);
    assert_eq!(snapshot.read_as::<i32>("r6"), 7);
    assert_eq!(snapshot.read("r7"), Duration::milliseconds(2000));
    assert_eq!(snapshot.read_as::<u64>("r8"), 0);
}

/// A field of a block lowers and compiles. A bit of a field is written with
/// the field addressed once (REQ-LOW-codegen-121).
#[rstest]
#[case::read("d := b.a;")]
#[case::read_output("x := b.q;")]
#[case::write("b.a := d;")]
#[case::write_constant("b.a := 7;")]
#[case::read_and_write("b.a := b.a + d;")]
#[case::bit_of_field("b.w.3 := x;")]
#[case::read_standard_block("d := c.CV;")]
#[case::read_standard_block_output("x := t.Q;")]
#[case::write_standard_block("c.PV := 7;")]
fn compile_when_block_field_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "{BLOCK}
         PROGRAM main
           VAR b : B; c : CTU; t : TON; d : DINT; x : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// A statement in the body of a block whose variables include an array,
/// references and a string, which the block's lowered type holds in the
/// order it declares them (REQ-LOW-lowering-170), lowers and compiles, as
/// does a read of the output the block declares after them.
#[rstest]
#[case::ref_of_field("own := REF(n);")]
#[case::through_reference_field("own^ := own^ + 1;")]
#[case::reference_copied("copy := src;")]
#[case::element_through_reference("n := src^[1];")]
#[case::element_through_copied_reference("copy^[0] := 99;")]
fn compile_when_block_field_of_any_type_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "FUNCTION_BLOCK H
           VAR_INPUT src : REF_TO ARRAY[0..4] OF INT; END_VAR
           VAR a : ARRAY[0..4] OF INT; own : REF_TO DINT; copy : REF_TO ARRAY[0..4] OF INT;
               n : DINT; s : STRING[8]; END_VAR
           VAR_OUTPUT o : DINT; END_VAR
           {statement}
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR h : H; d : DINT; END_VAR
           d := h.o;
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// A block's string declared through a type alias has a run of the data
/// region, as one declared `STRING[20]` does, and an assignment to it in the
/// block's body lowers and compiles (REQ-LOW-codegen-199).
#[test]
fn compile_when_block_string_through_alias_then_compiles() {
    assert_compiles(
        "TYPE Name : STRING[20]; END_TYPE
         FUNCTION_BLOCK B
           VAR s : Name; END_VAR
           s := 'abc';
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR f : B; END_VAR
           f();
         END_PROGRAM",
    );
}

/// A block with string, wide-string, structure and array fields, whose body
/// reads none of them: this backend does not reach what such a field of
/// another instance holds. The string fields of a block hold no string yet
/// (issue 1753), and a structure or array field holds the offset of the
/// region stored in it.
const AGGREGATES: &str = "
    TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
    FUNCTION_BLOCK S
      VAR_INPUT s : STRING; w : WSTRING; p : P; a : ARRAY[1..2] OF DINT; n : DINT; END_VAR
      VAR_OUTPUT qs : STRING; qw : WSTRING; q : DINT; END_VAR
      q := n + 1;
    END_FUNCTION_BLOCK
    FUNCTION LENGTH : DINT
      VAR_INPUT text : STRING; END_VAR
      LENGTH := LEN(text);
    END_FUNCTION";

/// A string, structure or array field of another instance, as a call's
/// input or output (REQ-LOW-lowering-069) or read or written outside a call,
/// lowers and compiles.
#[rstest]
#[case::string_input("f(s := t);")]
#[case::wide_string_input("f(w := u);")]
#[case::string_output("f(qs => t);")]
#[case::wide_string_output("f(qw => u);")]
#[case::string_literal_input("f(s := 'lit');")]
#[case::string_operation_input("f(s := CONCAT(t, t));")]
#[case::string_input_from_field("f(s := g.qs);")]
#[case::structure_input("f(p := v);")]
#[case::array_input("f(a := arr);")]
#[case::structure_input_from_field("f(p := g.p);")]
#[case::every_input_and_output(
    "f(s := t, w := u, p := v, a := arr, n := 1, qs => t, qw => u, q => r);"
)]
#[case::string_read("t := f.qs;")]
#[case::string_written("f.s := t;")]
#[case::string_literal_written("f.s := 'lit';")]
#[case::string_field_to_field("g.s := f.qs;")]
#[case::string_length("r := LEN(f.qs);")]
#[case::string_operand("t := CONCAT(f.qs, t);")]
#[case::string_compared("IF f.qs = t THEN r := 1; END_IF;")]
#[case::string_argument("r := LENGTH(text := f.qs);")]
#[case::string_in_subscript("arr[LEN(f.qs)] := 1;")]
#[case::structure_written("f.p := v;")]
#[case::array_written("f.a := arr;")]
#[case::structure_field_to_field("g.p := f.p;")]
fn compile_when_string_or_aggregate_field_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "{AGGREGATES}
         PROGRAM main
           VAR f : S; g : S; t : STRING := 'b'; u : WSTRING := \"c\"; v : P := (x := 5);
               arr : ARRAY[1..2] OF DINT; r : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// Calls that pass strings, a structure and an array, and a later write to
/// the arguments, leave every variable with the same values on every
/// backend, and the scalar input and output pass as they do for a block with
/// only scalar fields.
#[test]
fn run_when_string_and_aggregate_inputs_passed_then_backends_agree() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{AGGREGATES}
         PROGRAM main
           VAR f : S; t : STRING := 'b'; v : P := (x := 5); arr : ARRAY[1..2] OF DINT;
               first : DINT; second : DINT; END_VAR
           f(s := t, p := v, a := arr, n := 1, qs => t, q => first);
           v.x := 9;
           arr[1] := 9;
           f.p := v;
           f(p := v, a := arr, n := first, q => second);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("first"), 2);
    assert_eq!(snapshot.read_as::<i32>("second"), 3);
}
