//! End-to-end tests for the field defaults of a STRUCT type declaration
//! (#1523): every variable of the type starts with them, unless its own
//! initializer sets the field.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

const MOTOR: &str = "
TYPE
  Motor : STRUCT
    speed : INT := 100;
    power : DINT := 7777;
    rate : LREAL := 2.5;
  END_STRUCT;
END_TYPE
";

#[test]
fn end_to_end_when_struct_type_declares_defaults_then_variable_starts_with_them() {
    let source = format!(
        "{MOTOR}
PROGRAM main
  VAR a : INT; b : DINT; c : LREAL; m : Motor; END_VAR
  a := m.speed;
  b := m.power;
  c := m.rate;
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 100);
    assert_eq!(bufs.vars[1].as_i32(), 7777);
    assert_eq!(bufs.vars[2].as_f64(), 2.5);
}

#[test]
fn end_to_end_when_variable_initializer_sets_field_then_overrides_default() {
    let source = format!(
        "{MOTOR}
PROGRAM main
  VAR a : INT; b : DINT; m : Motor := (speed := 5); END_VAR
  a := m.speed;
  b := m.power;
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 7777);
}

#[test]
fn end_to_end_when_nested_struct_has_defaults_then_inner_fields_start_with_them() {
    let source = "
TYPE
  Inner : STRUCT a : INT := 3; b : INT := 4; END_STRUCT;
  Outer : STRUCT inner : Inner; c : INT := 5; END_STRUCT;
END_TYPE
PROGRAM main
  VAR x : INT; y : INT; z : INT; o : Outer := (inner := (a := 30)); END_VAR
  x := o.inner.a;
  y := o.inner.b;
  z := o.c;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 30);
    assert_eq!(bufs.vars[1].as_i32(), 4);
    assert_eq!(bufs.vars[2].as_i32(), 5);
}

#[test]
fn end_to_end_when_nested_field_default_is_a_structure_value_then_used() {
    let source = "
TYPE
  Inner : STRUCT a : INT; b : INT; END_STRUCT;
  Outer : STRUCT inner : Inner := (a := 7, b := 8); END_STRUCT;
END_TYPE
PROGRAM main
  VAR x : INT; y : INT; o : Outer; END_VAR
  x := o.inner.a;
  y := o.inner.b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 8);
}

#[test]
fn end_to_end_when_enum_and_bool_field_defaults_then_used() {
    let source = "
TYPE
  Color : (Red, Green, Blue);
  Lamp : STRUCT color : Color := Blue; lit : BOOL := TRUE; END_STRUCT;
END_TYPE
PROGRAM main
  VAR x : INT; l : Lamp; END_VAR
  IF l.lit AND l.color = Blue THEN
    x := 1;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 1);
}
