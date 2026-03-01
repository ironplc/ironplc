//! End-to-end integration tests for the ABS function.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_abs_positive_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 42;
  y := ABS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 42);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_abs_negative_then_positive() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := -7;
  y := ABS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), -7);
    assert_eq!(bufs.vars[1].as_i32(), 7);
}
