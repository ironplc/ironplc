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
    // Codegen doesn't yet support STRING struct field instantiation,
    // but defining the type should not block compilation.
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
