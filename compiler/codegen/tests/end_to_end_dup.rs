//! End-to-end execution tests verifying DUP optimizations produce correct results.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_var_squared_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := x * x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 25);
}

#[test]
fn end_to_end_when_store_load_optimized_then_correct_values() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 7;
  y := x + 3;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 10);
}

#[test]
fn end_to_end_when_chain_of_assignments_then_all_correct() {
    let source = "
PROGRAM main
  VAR
    a : DINT;
    b : DINT;
    c : DINT;
  END_VAR
  a := 10;
  b := a + 5;
  c := b * 2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 15);
    assert_eq!(bufs.vars[2].as_i32(), 30);
}

#[test]
fn end_to_end_when_var_doubled_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 7;
  y := x + x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 14);
}
