//! End-to-end integration tests for the NOT operator.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_not_zero_then_all_ones() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 0;
  y := NOT x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 0);
    assert_eq!(bufs.vars[1].as_i32(), -1);
}

#[test]
fn end_to_end_when_not_positive_then_complement() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 1;
  y := NOT x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    // NOT 1 = -2 (bitwise complement)
    assert_eq!(bufs.vars[1].as_i32(), -2);
}
