//! End-to-end integration tests for structure field read support.
//! Compiles ST programs with struct field access and runs them through the VM.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_struct_field_read_then_returns_initialized_value() {
    let source = "
TYPE MyStruct :
  STRUCT
    a : INT;
    b : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct := (a := 10, b := 20);
    result : DINT;
  END_VAR
    result := s.b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // result is var index 1 (s is var 0, result is var 1)
    assert_eq!(bufs.vars[1].as_i32(), 20);
}

#[test]
fn end_to_end_when_struct_field_read_first_field_then_correct_value() {
    let source = "
TYPE MyStruct :
  STRUCT
    a : INT;
    b : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct := (a := 10, b := 20);
    result : INT;
  END_VAR
    result := s.a;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 10);
}

#[test]
fn end_to_end_when_struct_field_arithmetic_then_correct_result() {
    let source = "
TYPE MyStruct :
  STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct := (x := 30, y := 12);
    result : DINT;
  END_VAR
    result := s.x + s.y;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_struct_field_read_default_init_then_returns_zero() {
    let source = "
TYPE MyStruct :
  STRUCT
    a : INT;
    b : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct;
    result : DINT;
  END_VAR
    result := s.b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_struct_field_read_bool_then_correct_value() {
    let source = "
TYPE MyStruct :
  STRUCT
    flag : BOOL;
    count : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct := (flag := TRUE, count := 5);
    result_flag : DINT;
    result_count : DINT;
  END_VAR
    result_flag := BOOL_TO_DINT(s.flag);
    result_count := s.count;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
    assert_eq!(bufs.vars[2].as_i32(), 5);
}
