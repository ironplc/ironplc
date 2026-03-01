//! End-to-end integration tests for the MIN function.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_min_then_returns_smaller() {
    let source = "
PROGRAM main
  VAR
    y : INT;
  END_VAR
  y := MIN(10, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 3);
}

#[test]
fn end_to_end_when_min_with_variable_then_returns_smaller() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 5;
  y := MIN(x, 100);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 5);
}
