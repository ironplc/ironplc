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

#[test]
fn end_to_end_when_user_function_calls_another_user_function_then_correct() {
    let source = "
FUNCTION INNER : DINT
  VAR_INPUT
    X : DINT;
  END_VAR
  INNER := X * 2;
END_FUNCTION

FUNCTION OUTER : DINT
  VAR_INPUT
    Y : DINT;
  END_VAR
  OUTER := INNER(X := Y);
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := OUTER(Y := 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // OUTER(3) calls INNER(3), which returns 3 * 2 = 6
    assert_eq!(bufs.vars[0].as_i32(), 6);
}

#[test]
fn end_to_end_when_unused_function_defined_then_result_unchanged() {
    // Without the unused function, the program computes OUTER(A:=3.0, B:=1.0)
    // which calls INNER(X:=3.0), returning 3.0 * 2.0 = 6.0, then adds 1.0 = 7.0.
    let source_without_unused = "
FUNCTION INNER : REAL
  VAR_INPUT
    X : REAL;
  END_VAR
  INNER := X * 2.0;
END_FUNCTION

FUNCTION OUTER : REAL
  VAR_INPUT
    A : REAL;
    B : REAL;
  END_VAR
  OUTER := INNER(X := A) + B;
END_FUNCTION

PROGRAM main
  VAR
    result : REAL;
  END_VAR
  result := OUTER(A := 3.0, B := 1.0);
END_PROGRAM
";

    // The same program but with an unused function that references an
    // undefined function. The compiler should tree-shake UNUSED_FUNC
    // so that it never reaches analysis or codegen.
    let source_with_unused = "
FUNCTION INNER : REAL
  VAR_INPUT
    X : REAL;
  END_VAR
  INNER := X * 2.0;
END_FUNCTION

FUNCTION UNUSED_FUNC : REAL
  VAR_INPUT
    X : REAL;
  END_VAR
  UNUSED_FUNC := X + 42.0;
END_FUNCTION

FUNCTION OUTER : REAL
  VAR_INPUT
    A : REAL;
    B : REAL;
  END_VAR
  OUTER := INNER(X := A) + B;
END_FUNCTION

PROGRAM main
  VAR
    result : REAL;
  END_VAR
  result := OUTER(A := 3.0, B := 1.0);
END_PROGRAM
";

    let (_c1, bufs1) = parse_and_run(source_without_unused);
    let (_c2, bufs2) = parse_and_run(source_with_unused);

    // Both should produce 7.0
    assert_eq!(bufs1.vars[0].as_f32(), 7.0);
    assert_eq!(bufs2.vars[0].as_f32(), 7.0);
}

#[test]
fn end_to_end_when_user_function_with_string_param_calls_len_then_returns_length() {
    let source = "
FUNCTION MY_LEN : INT
VAR_INPUT
    S : STRING;
END_VAR
    MY_LEN := LEN(S);
END_FUNCTION
PROGRAM main
VAR
    result : INT;
END_VAR
    result := MY_LEN(S := 'Hello');
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 5);
}

#[test]
fn end_to_end_when_user_function_with_string_param_from_variable_then_correct() {
    let source = "
FUNCTION MY_LEN : INT
VAR_INPUT
    S : STRING;
END_VAR
    MY_LEN := LEN(S);
END_FUNCTION
PROGRAM main
VAR
    greeting : STRING := 'Hi there';
    result : INT;
END_VAR
    result := MY_LEN(S := greeting);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // 'Hi there' has 8 characters.
    assert_eq!(bufs.vars[1].as_i32(), 8);
}

#[test]
fn end_to_end_when_user_function_with_string_and_scalar_params_then_correct() {
    let source = "
FUNCTION CHECK_LEN : INT
VAR_INPUT
    S : STRING;
    expected : INT;
END_VAR
VAR
    actual : INT;
END_VAR
    actual := LEN(S);
    IF actual = expected THEN
        CHECK_LEN := 1;
    ELSE
        CHECK_LEN := 0;
    END_IF;
END_FUNCTION
PROGRAM main
VAR
    result : INT;
END_VAR
    result := CHECK_LEN(S := 'ABC', expected := 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_user_function_with_string_param_called_twice_then_both_correct() {
    let source = "
FUNCTION MY_LEN : INT
VAR_INPUT
    S : STRING;
END_VAR
    MY_LEN := LEN(S);
END_FUNCTION
PROGRAM main
VAR
    r1 : INT;
    r2 : INT;
END_VAR
    r1 := MY_LEN(S := 'AB');
    r2 := MY_LEN(S := 'ABCDE');
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 2);
    assert_eq!(bufs.vars[1].as_i32(), 5);
}
