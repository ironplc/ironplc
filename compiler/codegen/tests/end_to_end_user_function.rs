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

#[test]
fn end_to_end_when_user_function_assigns_return_var_then_uses_in_builtin_then_correct() {
    let source = "
FUNCTION FOO : INT
  VAR_INPUT
    A : INT;
  END_VAR
  FOO := 8;
  FOO := SHR(FOO, 1);
END_FUNCTION

PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := FOO(A := 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // FOO assigns 8, then shifts right by 1: 8 >> 1 = 4
    assert_eq!(bufs.vars[0].as_i32(), 4);
}

#[test]
fn end_to_end_when_function_called_twice_then_locals_reinitialized() {
    // The motivating example from ADR-0024: a function with a local variable
    // that has an initial value must re-initialize on every call.
    let source = "
FUNCTION accumulate : DINT
  VAR_INPUT a : DINT; END_VAR
  VAR counter : DINT := 10; END_VAR
  counter := counter + a;
  accumulate := counter;
END_FUNCTION

PROGRAM main
  VAR
    r1 : DINT;
    r2 : DINT;
  END_VAR
  r1 := accumulate(5);
  r2 := accumulate(3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // First call: counter starts at 10, adds 5 → 15
    assert_eq!(bufs.vars[0].as_i32(), 15);
    // Second call: counter must restart at 10 (not 15), adds 3 → 13
    assert_eq!(bufs.vars[1].as_i32(), 13);
}

#[test]
fn end_to_end_when_function_called_twice_then_zero_default_locals_reinitialized() {
    // A function with a local variable that has no explicit initializer
    // must be zero-initialized on every call.
    let source = "
FUNCTION sum_via_local : DINT
  VAR_INPUT x : DINT; END_VAR
  VAR accum : DINT; END_VAR
  accum := accum + x;
  sum_via_local := accum;
END_FUNCTION

PROGRAM main
  VAR
    r1 : DINT;
    r2 : DINT;
  END_VAR
  r1 := sum_via_local(7);
  r2 := sum_via_local(3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // First call: accum starts at 0, adds 7 → 7
    assert_eq!(bufs.vars[0].as_i32(), 7);
    // Second call: accum must restart at 0 (not 7), adds 3 → 3
    assert_eq!(bufs.vars[1].as_i32(), 3);
}

#[test]
fn end_to_end_when_function_called_twice_then_return_value_reinitialized() {
    // The return variable must also be zero-initialized on every call.
    // If it retained its value, the second call would see the first call's result.
    let source = "
FUNCTION conditional_set : DINT
  VAR_INPUT flag : DINT; END_VAR
  IF flag > 0 THEN
    conditional_set := 42;
  END_IF;
END_FUNCTION

PROGRAM main
  VAR
    r1 : DINT;
    r2 : DINT;
  END_VAR
  r1 := conditional_set(1);
  r2 := conditional_set(0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // First call: flag > 0, so return 42
    assert_eq!(bufs.vars[0].as_i32(), 42);
    // Second call: flag = 0, so return value stays at default (0), not stale 42
    assert_eq!(bufs.vars[1].as_i32(), 0);
}
