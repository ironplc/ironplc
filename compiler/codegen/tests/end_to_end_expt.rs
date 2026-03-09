//! End-to-end integration tests for EXPT with DINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_expt_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := EXPT(2, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 1024);
}

#[test]
fn end_to_end_when_expt_zero_exponent_then_one() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := EXPT(5, 0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 1);
}
