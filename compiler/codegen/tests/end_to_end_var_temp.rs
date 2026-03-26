//! End-to-end integration tests for VAR_TEMP declarations.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_function_with_var_temp_then_correct() {
    let source = "
FUNCTION add_doubled : DINT
  VAR_INPUT
    a : DINT;
    b : DINT;
  END_VAR
  VAR_TEMP
    temp : DINT;
  END_VAR
  temp := a + b;
  add_doubled := temp * 2;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := add_doubled(3, 4);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 14);
}

#[test]
fn end_to_end_when_function_with_multiple_var_temp_then_correct() {
    let source = "
FUNCTION compute : DINT
  VAR_INPUT
    x : DINT;
  END_VAR
  VAR_TEMP
    a : DINT;
    b : DINT;
  END_VAR
  a := x + 1;
  b := a * 3;
  compute := b;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := compute(4);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 15);
}
