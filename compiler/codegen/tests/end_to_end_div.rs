//! End-to-end integration tests for the DIV operator.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_div_expression_then_variable_has_quotient() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 12;
  y := x / 4;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 12);
    assert_eq!(bufs.vars[1].as_i32(), 3);
}

#[test]
fn end_to_end_when_chain_of_divisions_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 100 / 5 / 2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // (100 / 5) / 2 = 10
    assert_eq!(bufs.vars[0].as_i32(), 10);
}
