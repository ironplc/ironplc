//! Tests of the fields of function block instances read and written from
//! the lowered program, with every statement required to lower.

use rstest::rstest;

use crate::common::Duration;
use crate::compile_route_lowered::{assert_same_code, run_lowered};

/// A block with fields of several types, which no test calls.
const BLOCK: &str = "
    FUNCTION_BLOCK B
      VAR_INPUT a : DINT; w : WORD; t : TIME; END_VAR
      VAR_OUTPUT q : BOOL; END_VAR
      q := a > 0;
    END_FUNCTION_BLOCK";

/// A field of a block is read and written outside a call, a member
/// initializer sets one, and a bit of a field is written and read.
#[test]
fn run_when_block_fields_read_and_written_then_values() {
    let snapshot = run_lowered(&format!(
        "{BLOCK}
         PROGRAM main
           VAR b : B := (t := T#250MS);
               r1 : DINT; r2 : BOOL; r3 : TIME; r4 : WORD; r5 : BOOL; END_VAR
           b.a := 41;
           b.a := b.a + 1;
           r1 := b.a;
           r2 := b.q;
           r3 := b.t;
           b.w.3 := TRUE;
           r4 := b.w;
           r5 := b.w.3;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r1"), 42);
    assert_eq!(snapshot.read_as::<u64>("r2"), 0);
    assert_eq!(snapshot.read("r3"), Duration::milliseconds(250));
    assert_eq!(snapshot.read_as::<u64>("r4"), 8);
    assert_eq!(snapshot.read_as::<u64>("r5"), 1);
}

/// A field of a block compiles to the same instructions and constants by
/// both routes. A bit of a field is left out: the AST route addresses the
/// field once to read it and again to write it, and the lowered route once
/// (REQ-LOW-codegen-121).
#[rstest]
#[case::read("d := b.a;")]
#[case::read_output("x := b.q;")]
#[case::write("b.a := d;")]
#[case::write_constant("b.a := 7;")]
#[case::read_and_write("b.a := b.a + d;")]
fn compile_when_block_field_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "{BLOCK}
         PROGRAM main
           VAR b : B; d : DINT; x : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}
