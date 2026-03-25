//! End-to-end integration tests for array support.
//! Compiles ST programs with arrays and runs them through the VM.

mod common;

use common::{parse_and_run, parse_and_run_rounds};

#[test]
fn end_to_end_when_array_store_and_load_then_roundtrips() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..5] OF INT;
    x : INT;
  END_VAR
  arr[3] := 42;
  x := arr[3];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // x is at var index 1 (arr is var 0, x is var 1)
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_array_sum_loop_then_computes_correct_sum() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..5] OF INT;
    sum : INT := 0;
    i : INT;
  END_VAR
  arr[1] := 10;
  arr[2] := 20;
  arr[3] := 30;
  arr[4] := 40;
  arr[5] := 50;

  FOR i := 1 TO 5 DO
    sum := sum + arr[i];
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // sum is var index 1 (arr=0, sum=1, i=2)
    assert_eq!(bufs.vars[1].as_i32(), 150);
}

#[test]
fn end_to_end_when_array_with_initialization_then_values_set() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF INT := [10, 20, 30];
    x : INT;
  END_VAR
  x := arr[2];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 20);
}

#[test]
fn end_to_end_when_array_dint_then_correct_values() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..2] OF DINT;
    x : DINT;
  END_VAR
  arr[0] := 100000;
  arr[1] := 200000;
  arr[2] := 300000;
  x := arr[1];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 200000);
}

#[test]
fn end_to_end_when_array_negative_lower_bound_then_correct_indexing() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[-2..2] OF INT;
    x : INT;
  END_VAR
  arr[-2] := 100;
  arr[-1] := 200;
  arr[0] := 300;
  arr[1] := 400;
  arr[2] := 500;
  x := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 300);
}

#[test]
fn end_to_end_when_array_multiple_independent_stores_then_no_interference() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF INT;
    a : INT;
    b : INT;
    c : INT;
  END_VAR
  arr[1] := 11;
  arr[2] := 22;
  arr[3] := 33;
  a := arr[1];
  b := arr[2];
  c := arr[3];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // arr=0, a=1, b=2, c=3
    assert_eq!(bufs.vars[1].as_i32(), 11);
    assert_eq!(bufs.vars[2].as_i32(), 22);
    assert_eq!(bufs.vars[3].as_i32(), 33);
}

#[test]
fn end_to_end_when_array_persists_across_scans_then_values_retained() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF INT;
    x : INT;
    scan_count : INT;
  END_VAR
  scan_count := scan_count + 1;
  IF scan_count = 1 THEN
    arr[1] := 99;
  END_IF;
  x := arr[1];
END_PROGRAM
";
    parse_and_run_rounds(source, |vm| {
        // First scan: sets arr[1] = 99
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(1).unwrap(), 99);

        // Second scan: arr[1] should still be 99
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(1).unwrap(), 99);
    });
}

#[test]
fn end_to_end_when_array_with_repeated_init_then_values_set() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..6] OF INT := [3(10), 3(20)];
    x : INT;
    y : INT;
  END_VAR
  x := arr[1];
  y := arr[4];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // arr=0, x=1, y=2
    assert_eq!(bufs.vars[1].as_i32(), 10); // arr[1] = first of 3(10)
    assert_eq!(bufs.vars[2].as_i32(), 20); // arr[4] = first of 3(20)
}

#[test]
fn end_to_end_when_array_2d_then_correct_indexing() {
    let source = "
PROGRAM main
  VAR
    matrix : ARRAY[1..3, 1..4] OF INT;
    x : INT;
  END_VAR
  matrix[2, 3] := 42;
  x := matrix[2, 3];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_array_in_function_var_then_parses_and_analyzes() {
    let source = "
FUNCTION MY_FUNC : INT
VAR_INPUT
    x : INT;
END_VAR
VAR
    stack : ARRAY[1..32] OF INT;
END_VAR
    MY_FUNC := x;
END_FUNCTION
PROGRAM main
VAR
    result : INT;
    arg : INT;
END_VAR
    arg := 42;
    result := MY_FUNC(x := arg);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 42);
}
