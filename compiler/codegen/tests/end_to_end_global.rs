//! End-to-end integration tests for global variables (VAR_GLOBAL + VAR_EXTERNAL).
//!
//! These tests verify that global variables declared in a CONFIGURATION block
//! are accessible from a PROGRAM via VAR_EXTERNAL declarations.

mod common;
use ironplc_parser::options::{CompilerOptions, Dialect};

use common::parse_and_run;

#[test]
fn end_to_end_when_global_var_with_initial_value_then_external_reads_value() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    shared : INT := 42;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    shared : INT;
  END_VAR
  VAR
    result : INT;
  END_VAR
  result := shared;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // shared is at index 0 (global), result is at index 1 (program local)
    assert_eq!(bufs.vars[0].as_i32(), 42);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_global_var_written_via_external_then_value_updated() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    counter : DINT := 0;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    counter : DINT;
  END_VAR
  counter := counter + 10;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // counter starts at 0, after one scan: 0 + 10 = 10
    assert_eq!(bufs.vars[0].as_i32(), 10);
}

#[test]
fn end_to_end_when_global_var_no_initial_value_then_defaults_to_zero() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    value : INT;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    value : INT;
  END_VAR
  VAR
    result : INT;
  END_VAR
  result := value;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 0);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_multiple_globals_then_all_accessible() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    a : INT := 10;
    b : DINT := 20;
    c : BOOL := TRUE;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    a : INT;
    b : DINT;
    c : BOOL;
  END_VAR
  VAR
    sum : DINT;
  END_VAR
  sum := a + b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // a=10, b=20, c=TRUE(1), sum=30
    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 20);
    assert_eq!(bufs.vars[2].as_i32(), 1);
    assert_eq!(bufs.vars[3].as_i32(), 30);
}

#[test]
fn end_to_end_when_global_constant_then_readable() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL CONSTANT
    max_value : INT := 100;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL CONSTANT
    max_value : INT;
  END_VAR
  VAR
    result : INT;
  END_VAR
  result := max_value;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 100);
    assert_eq!(bufs.vars[1].as_i32(), 100);
}

#[test]
fn end_to_end_when_global_array_then_external_can_read_elements() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    readings : ARRAY[1..3] OF INT := [10, 20, 30];
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    readings : ARRAY[1..3] OF INT;
  END_VAR
  VAR
    first : INT;
    second : INT;
    third : INT;
  END_VAR
  first := readings[1];
  second := readings[2];
  third := readings[3];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // readings is at index 0 (global array), first/second/third at 1/2/3
    assert_eq!(bufs.vars[1].as_i32(), 10);
    assert_eq!(bufs.vars[2].as_i32(), 20);
    assert_eq!(bufs.vars[3].as_i32(), 30);
}

#[test]
fn end_to_end_when_global_array_then_external_can_write_elements() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    data : ARRAY[1..3] OF DINT := [0, 0, 0];
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    data : ARRAY[1..3] OF DINT;
  END_VAR
  VAR
    result : DINT;
  END_VAR
  data[1] := 100;
  data[2] := 200;
  data[3] := 300;
  result := data[1] + data[2] + data[3];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // result should be 100 + 200 + 300 = 600
    assert_eq!(bufs.vars[1].as_i32(), 600);
}

#[test]
fn end_to_end_when_no_configuration_then_program_still_works() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 99;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 99);
}

#[test]
fn end_to_end_when_top_level_global_struct_then_field_readable_from_program() {
    let source = "
TYPE MY_CONSTANTS :
  STRUCT
    T0 : DINT;
    T1 : DINT;
  END_STRUCT;
END_TYPE

VAR_GLOBAL
  phys : MY_CONSTANTS;
END_VAR

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  phys.T0 := 100;
  phys.T1 := 200;
  result := phys.T0 + phys.T1;
END_PROGRAM
";
    // Rusty dialect prepends 2 system uptime globals, so:
    // var 0: __SYSTEM_UP_TIME, var 1: __SYSTEM_UP_LTIME,
    // var 2: phys (global struct), var 3: result (program local)
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::from_dialect(Dialect::Rusty));

    assert_eq!(bufs.vars[3].as_i32(), 300);
}

#[test]
fn end_to_end_when_top_level_global_struct_then_field_readable_from_function() {
    let source = "
TYPE MY_CONSTANTS :
  STRUCT
    T0 : DINT;
  END_STRUCT;
END_TYPE

VAR_GLOBAL
  phys : MY_CONSTANTS;
END_VAR

FUNCTION GET_T0 : DINT
VAR_INPUT
  dummy : DINT;
END_VAR
  GET_T0 := phys.T0;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  phys.T0 := 273;
  result := GET_T0(dummy := 0);
END_PROGRAM
";
    // var 0: __SYSTEM_UP_TIME, var 1: __SYSTEM_UP_LTIME,
    // var 2: phys (global struct), var 3: result (program local)
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::from_dialect(Dialect::Rusty));

    assert_eq!(bufs.vars[3].as_i32(), 273);
}

#[test]
fn end_to_end_when_top_level_global_scalar_then_readable_from_program() {
    let source = "
VAR_GLOBAL
  counter : DINT := 42;
END_VAR

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := counter;
END_PROGRAM
";
    // var 0: __SYSTEM_UP_TIME, var 1: __SYSTEM_UP_LTIME,
    // var 2: counter (global), var 3: result (program local)
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::from_dialect(Dialect::Rusty));

    assert_eq!(bufs.vars[2].as_i32(), 42);
    assert_eq!(bufs.vars[3].as_i32(), 42);
}
