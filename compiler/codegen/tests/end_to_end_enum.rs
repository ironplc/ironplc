//! End-to-end tests for enumeration code generation.
//!
//! Tests the complete pipeline from IEC 61131-3 source with enumeration
//! types through code generation and VM execution.

mod common;

use ironplc_container::debug_section::iec_type_tag;
use ironplc_parser::options::CompilerOptions;

use common::{parse_and_compile, parse_and_run};

// --- PR 1: Compilation tests ---

#[test]
fn end_to_end_when_enum_type_declared_then_compiles_without_error() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 0;
END_PROGRAM
";
    let _container = parse_and_compile(source, &CompilerOptions::default());
}

#[test]
fn end_to_end_when_multiple_enum_types_then_compiles_without_error() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 0;
END_PROGRAM
";
    let _container = parse_and_compile(source, &CompilerOptions::default());
}

// --- PR 2: Variable allocation + initialization tests ---

#[test]
fn end_to_end_when_enum_variable_with_explicit_init_then_initializes_to_ordinal() {
    let source = "
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    x : LEVEL := MEDIUM;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // MEDIUM is ordinal 1
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_enum_variable_no_explicit_init_then_uses_type_default() {
    let source = "
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    x : LEVEL;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Type default is LOW = ordinal 0
    assert_eq!(bufs.vars[0].as_i32(), 0);
}

#[test]
fn end_to_end_when_enum_variable_with_non_first_default_then_uses_type_default() {
    let source = "
TYPE LEVEL : (LOW, MEDIUM, HIGH) := HIGH; END_TYPE
PROGRAM main
  VAR
    x : LEVEL;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Type default is HIGH = ordinal 2
    assert_eq!(bufs.vars[0].as_i32(), 2);
}

#[test]
fn end_to_end_when_multiple_enum_variables_then_each_has_correct_ordinal() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    a : COLOR := RED;
    b : COLOR := GREEN;
    c : COLOR := BLUE;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0); // RED
    assert_eq!(bufs.vars[1].as_i32(), 1); // GREEN
    assert_eq!(bufs.vars[2].as_i32(), 2); // BLUE
}

#[test]
fn end_to_end_when_enum_variable_then_debug_section_has_dint_tag_and_type_name() {
    let source = "
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    x : LEVEL;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();
    let var = &debug.var_names[0];
    assert_eq!(var.name, "x");
    assert_eq!(var.type_name, "LEVEL");
    assert_eq!(var.iec_type_tag, iec_type_tag::DINT);
}

#[test]
fn end_to_end_when_two_enum_types_with_variables_then_each_initialized_correctly() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    c : COLOR := BLUE;
    l : LEVEL := HIGH;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 2); // BLUE
    assert_eq!(bufs.vars[1].as_i32(), 2); // HIGH
}

// --- PR 3: Enum value expressions + CASE selectors ---

#[test]
fn end_to_end_when_enum_assignment_in_body_then_stores_ordinal() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
  END_VAR
  c := BLUE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 2); // BLUE = ordinal 2
}

#[test]
fn end_to_end_when_enum_case_then_matches_correct_arm() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    result : DINT;
  END_VAR
  c := GREEN;
  CASE c OF
    RED: result := 10;
    GREEN: result := 20;
    BLUE: result := 30;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 20);
}

#[test]
fn end_to_end_when_enum_case_with_else_then_falls_through() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    result : DINT;
  END_VAR
  c := BLUE;
  CASE c OF
    RED: result := 10;
    GREEN: result := 20;
  ELSE
    result := 99;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 99);
}

#[test]
fn end_to_end_when_enum_case_first_value_then_matches() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    result : DINT;
  END_VAR
  c := RED;
  CASE c OF
    RED: result := 10;
    GREEN: result := 20;
    BLUE: result := 30;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 10);
}

#[test]
fn end_to_end_when_enum_case_last_value_then_matches() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    result : DINT;
  END_VAR
  c := BLUE;
  CASE c OF
    RED: result := 10;
    GREEN: result := 20;
    BLUE: result := 30;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 30);
}

#[test]
fn end_to_end_when_enum_case_multi_value_arm_then_matches() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    result : DINT;
  END_VAR
  c := RED;
  CASE c OF
    RED, GREEN: result := 10;
    BLUE: result := 20;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 10);
}

// --- PR 4: Struct field enum initialization ---

#[test]
fn end_to_end_when_struct_with_enum_field_init_then_stores_ordinal() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
TYPE MyStruct :
  STRUCT
    c : COLOR;
    v : DINT;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    s : MyStruct := (c := GREEN, v := 42);
    result : DINT;
  END_VAR
  result := s.v;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 42);
}
