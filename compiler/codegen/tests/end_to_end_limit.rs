//! End-to-end integration tests for the LIMIT function.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_limit_in_range_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 5;
  y := LIMIT(0, x, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 5);
}

#[test]
fn end_to_end_when_limit_below_min_then_clamped() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := -5;
  y := LIMIT(0, x, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), -5);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_limit_above_max_then_clamped() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 15;
  y := LIMIT(0, x, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 15);
    assert_eq!(bufs.vars[1].as_i32(), 10);
}
