//! End-to-end integration tests for user-defined function calls.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_user_function_add_then_returns_sum() {
    let source = "
FUNCTION add_two : DINT
  VAR_INPUT
    a : DINT;
    b : DINT;
  END_VAR
  add_two := a + b;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := add_two(3, 7);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 10);
}

#[test]
fn end_to_end_when_user_function_with_local_var_then_correct() {
    let source = "
FUNCTION double_plus_one : DINT
  VAR_INPUT
    x : DINT;
  END_VAR
  VAR
    temp : DINT;
  END_VAR
  temp := x * 2;
  double_plus_one := temp + 1;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := double_plus_one(5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 11);
}

#[test]
fn end_to_end_when_user_function_called_twice_then_both_correct() {
    let source = "
FUNCTION square : DINT
  VAR_INPUT
    n : DINT;
  END_VAR
  square := n * n;
END_FUNCTION

PROGRAM main
  VAR
    a : DINT;
    b : DINT;
  END_VAR
  a := square(3);
  b := square(5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 9);
    assert_eq!(bufs.vars[1].as_i32(), 25);
}

#[test]
fn end_to_end_when_user_function_in_expression_then_correct() {
    let source = "
FUNCTION inc : DINT
  VAR_INPUT
    x : DINT;
  END_VAR
  inc := x + 1;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := inc(10) + inc(20);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 32);
}
