//! End-to-end integration tests for comparison operators.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_eq_true_then_one() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 5;
  y := x = 5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_ne_true_then_one() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 5;
  y := x <> 3;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_lt_true_then_one() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 3;
  y := x < 5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 3);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_le_equal_then_one() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 5;
  y := x <= 5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_gt_true_then_one() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 7;
  y := x > 5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_ge_false_then_zero() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 3;
  y := x >= 5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 3);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}
