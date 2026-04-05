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

#[test]
fn end_to_end_when_struct_with_string_field_defined_then_program_runs() {
    // Struct with STRING field is defined but not instantiated.
    let source = "
TYPE MY_DATA :
  STRUCT
    NAME : STRING;
    VALUE : INT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    x : INT;
  END_VAR
    x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_global_struct_with_string_field_then_compiles_and_runs() {
    // Regression test: global struct with STRING field previously failed with
    // P9999 "Structure contains unsupported field types".
    let source = "
TYPE MY_DATA :
  STRUCT
    NAME : STRING[30];
    VALUE : INT;
  END_STRUCT;
END_TYPE

VAR_GLOBAL
    data1 : MY_DATA;
END_VAR

PROGRAM main
  VAR
    x : INT;
  END_VAR
    x := 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // data1 is var 0 (global), x is var 1
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_local_struct_with_string_field_then_compiles_and_runs() {
    // Struct with STRING field as local variable.
    let source = "
TYPE MY_DATA :
  STRUCT
    NAME : STRING[30];
    VALUE : INT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    data1 : MY_DATA;
    x : INT;
  END_VAR
    x := 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // x is var index 1 (data1 is var 0)
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_struct_with_string_field_then_int_field_accessible() {
    // Read the INT field of a struct that also contains a STRING field.
    let source = "
TYPE MY_DATA :
  STRUCT
    NAME : STRING[30];
    VALUE : INT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    data1 : MY_DATA;
    result : INT;
  END_VAR
    data1.VALUE := 42;
    result := data1.VALUE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // result is var index 1
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_struct_field_write_then_value_stored() {
    let source = "
TYPE MY_POINT :
  STRUCT
    X : REAL;
    Y : REAL;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    pt : MY_POINT;
    result : REAL;
  END_VAR
    pt.X := 1.0;
    result := pt.X;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_f32(), 1.0);
}

#[test]
fn end_to_end_when_struct_field_write_both_fields_then_correct_values() {
    let source = "
TYPE MY_POINT :
  STRUCT
    X : REAL;
    Y : REAL;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    pt : MY_POINT;
    rx : REAL;
    ry : REAL;
  END_VAR
    pt.X := 1.0;
    pt.Y := 2.0;
    rx := pt.X;
    ry := pt.Y;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_f32(), 1.0);
    assert_eq!(bufs.vars[2].as_f32(), 2.0);
}

#[test]
fn end_to_end_when_struct_field_write_int_then_correct_value() {
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
    s.a := 42;
    s.b := 100;
    result := s.a + s.b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 142);
}

#[test]
fn end_to_end_when_struct_array_field_read_constant_index_then_correct_element() {
    let source = "
TYPE MyStruct :
  STRUCT
    values : ARRAY[0..2] OF DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct;
    result : DINT;
  END_VAR
    s.values[0] := 10;
    s.values[1] := 20;
    s.values[2] := 30;
    result := s.values[1];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 20);
}

#[test]
fn end_to_end_when_struct_array_field_write_then_stores_value() {
    let source = "
TYPE MyStruct :
  STRUCT
    data : ARRAY[1..3] OF DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct;
    result : DINT;
  END_VAR
    s.data[1] := 100;
    s.data[2] := 200;
    s.data[3] := 300;
    result := s.data[1] + s.data[2] + s.data[3];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 600);
}

#[test]
fn end_to_end_when_struct_array_field_variable_index_then_correct() {
    let source = "
TYPE MyStruct :
  STRUCT
    items : ARRAY[0..4] OF REAL;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct;
    i : INT;
    result : REAL;
  END_VAR
    s.items[0] := 1.0;
    s.items[1] := 2.0;
    s.items[2] := 3.0;
    s.items[3] := 4.0;
    s.items[4] := 5.0;
    i := 3;
    result := s.items[i];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_f32(), 4.0);
}

#[test]
fn end_to_end_when_struct_with_scalar_and_array_fields_then_both_correct() {
    let source = "
TYPE Mixed :
  STRUCT
    count : DINT;
    values : ARRAY[0..2] OF DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    m : Mixed;
    result : DINT;
  END_VAR
    m.count := 3;
    m.values[0] := 10;
    m.values[1] := 20;
    m.values[2] := 30;
    result := m.count + m.values[0] + m.values[1] + m.values[2];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 63);
}

#[test]
fn end_to_end_when_function_returns_struct_with_field_assignment_then_fields_correct() {
    let source = "
TYPE POINT :
  STRUCT
    X : REAL;
    Y : REAL;
  END_STRUCT;
END_TYPE

FUNCTION MAKE_POINT : POINT
VAR_INPUT
    px : REAL;
    py : REAL;
END_VAR
    MAKE_POINT.X := px;
    MAKE_POINT.Y := py;
END_FUNCTION

PROGRAM main
  VAR
    p : POINT;
    rx : REAL;
    ry : REAL;
  END_VAR
    p := MAKE_POINT(px := 1.5, py := 2.5);
    rx := p.X;
    ry := p.Y;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // p is var 0 (struct), rx is var 1, ry is var 2
    assert_eq!(bufs.vars[1].as_f32(), 1.5);
    assert_eq!(bufs.vars[2].as_f32(), 2.5);
}

#[test]
fn end_to_end_when_two_calls_to_struct_returning_function_then_independent_copies() {
    let source = "
TYPE POINT :
  STRUCT
    X : REAL;
    Y : REAL;
  END_STRUCT;
END_TYPE

FUNCTION MAKE_POINT : POINT
VAR_INPUT
    px : REAL;
    py : REAL;
END_VAR
    MAKE_POINT.X := px;
    MAKE_POINT.Y := py;
END_FUNCTION

PROGRAM main
  VAR
    p1 : POINT;
    p2 : POINT;
    r1x : REAL;
    r1y : REAL;
    r2x : REAL;
    r2y : REAL;
  END_VAR
    p1 := MAKE_POINT(px := 1.0, py := 2.0);
    p2 := MAKE_POINT(px := 3.0, py := 4.0);
    r1x := p1.X;
    r1y := p1.Y;
    r2x := p2.X;
    r2y := p2.Y;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // p1 is var 0, p2 is var 1, r1x is var 2, r1y is var 3, r2x is var 4, r2y is var 5
    assert_eq!(bufs.vars[2].as_f32(), 1.0);
    assert_eq!(bufs.vars[3].as_f32(), 2.0);
    assert_eq!(bufs.vars[4].as_f32(), 3.0);
    assert_eq!(bufs.vars[5].as_f32(), 4.0);
}
