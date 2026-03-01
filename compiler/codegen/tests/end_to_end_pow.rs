//! End-to-end integration tests for the POW/EXPT operator.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_pow_expression_then_variable_has_power() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 3;
  y := x ** 4;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 3);
    assert_eq!(bufs.vars[1].as_i32(), 81); // 3^4 = 81
}

#[test]
fn end_to_end_when_pow_with_zero_exponent_then_one() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 7;
  y := x ** 0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 1); // 7^0 = 1
}
