//! End-to-end tests for enumeration code generation.
//!
//! Tests the complete pipeline from IEC 61131-3 source with enumeration
//! types through code generation and VM execution.

#[macro_use]
mod common;

use common::parse_and_compile;
use ironplc_container::debug_section::iec_type_tag;
use ironplc_parser::options::CompilerOptions;

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

// MEDIUM is ordinal 1.
e2e_i32!(
    end_to_end_when_enum_variable_with_explicit_init_then_initializes_to_ordinal,
    "TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE PROGRAM main VAR x : LEVEL := MEDIUM; END_VAR END_PROGRAM",
    &[(0, 1)],
);

// Type default is LOW = ordinal 0.
e2e_i32!(
    end_to_end_when_enum_variable_no_explicit_init_then_uses_type_default,
    "TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE PROGRAM main VAR x : LEVEL; END_VAR END_PROGRAM",
    &[(0, 0)],
);

// Type default is HIGH = ordinal 2.
e2e_i32!(
    end_to_end_when_enum_variable_with_non_first_default_then_uses_type_default,
    "TYPE LEVEL : (LOW, MEDIUM, HIGH) := HIGH; END_TYPE PROGRAM main VAR x : LEVEL; END_VAR END_PROGRAM",
    &[(0, 2)],
);

// RED = 0, GREEN = 1, BLUE = 2.
e2e_i32!(
    end_to_end_when_multiple_enum_variables_then_each_has_correct_ordinal,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR a : COLOR := RED; b : COLOR := GREEN; c : COLOR := BLUE; END_VAR END_PROGRAM",
    &[(0, 0), (1, 1), (2, 2)],
);

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

// BLUE = 2, HIGH = 2.
e2e_i32!(
    end_to_end_when_two_enum_types_with_variables_then_each_initialized_correctly,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE PROGRAM main VAR c : COLOR := BLUE; l : LEVEL := HIGH; END_VAR END_PROGRAM",
    &[(0, 2), (1, 2)],
);

// --- PR 3: Enum value expressions + CASE selectors ---

// BLUE = ordinal 2.
e2e_i32!(
    end_to_end_when_enum_assignment_in_body_then_stores_ordinal,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR c : COLOR; END_VAR c := BLUE; END_PROGRAM",
    &[(0, 2)],
);

e2e_i32!(
    end_to_end_when_enum_case_then_matches_correct_arm,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR c : COLOR; result : DINT; END_VAR c := GREEN; CASE c OF RED: result := 10; GREEN: result := 20; BLUE: result := 30; END_CASE; END_PROGRAM",
    &[(1, 20)],
);

e2e_i32!(
    end_to_end_when_enum_case_with_else_then_falls_through,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR c : COLOR; result : DINT; END_VAR c := BLUE; CASE c OF RED: result := 10; GREEN: result := 20; ELSE result := 99; END_CASE; END_PROGRAM",
    &[(1, 99)],
);

e2e_i32!(
    end_to_end_when_enum_case_first_value_then_matches,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR c : COLOR; result : DINT; END_VAR c := RED; CASE c OF RED: result := 10; GREEN: result := 20; BLUE: result := 30; END_CASE; END_PROGRAM",
    &[(1, 10)],
);

e2e_i32!(
    end_to_end_when_enum_case_last_value_then_matches,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR c : COLOR; result : DINT; END_VAR c := BLUE; CASE c OF RED: result := 10; GREEN: result := 20; BLUE: result := 30; END_CASE; END_PROGRAM",
    &[(1, 30)],
);

e2e_i32!(
    end_to_end_when_enum_case_multi_value_arm_then_matches,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR c : COLOR; result : DINT; END_VAR c := RED; CASE c OF RED, GREEN: result := 10; BLUE: result := 20; END_CASE; END_PROGRAM",
    &[(1, 10)],
);

// --- PR 4: Struct field enum initialization ---

e2e_i32!(
    end_to_end_when_struct_with_enum_field_init_then_stores_ordinal,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE TYPE MyStruct : STRUCT c : COLOR; v : DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct := (c := GREEN, v := 42); result : DINT; END_VAR result := s.v; END_PROGRAM",
    &[(1, 42)],
);

// --- PR 4.5: Enum comparison in expressions ---

e2e_i32!(
    end_to_end_when_enum_comparison_in_if_then_correct_result,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR c : COLOR; result : DINT; END_VAR c := GREEN; IF c = GREEN THEN result := 42; END_IF; END_PROGRAM",
    &[(1, 42)],
);

e2e_i32!(
    end_to_end_when_enum_comparison_not_equal_then_skips,
    "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE PROGRAM main VAR c : COLOR; result : DINT; END_VAR c := RED; IF c = GREEN THEN result := 42; END_IF; END_PROGRAM",
    &[(1, 0)],
);

// State=RUNNING (TRUE) AND Seal=TRUE -> CONTACTOR=TRUE (1).
e2e_i32!(
    end_to_end_when_enum_comparison_in_bool_expression_then_correct_result,
    "TYPE MotorState : (STOPPED, RUNNING, FAULTED) := STOPPED; END_TYPE PROGRAM main VAR State : MotorState; Seal : BOOL; CONTACTOR : BOOL; END_VAR State := RUNNING; Seal := TRUE; CONTACTOR := (State = RUNNING) AND Seal; END_PROGRAM",
    &[(2, 1)],
);

// --- PR 5: Debug section enum definitions ---

#[test]
fn end_to_end_when_enum_type_then_debug_section_has_enum_def() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();
    let color_def = debug
        .enum_defs
        .iter()
        .find(|e| e.type_name == "COLOR")
        .expect("COLOR enum def should be present");
    assert_eq!(color_def.values, vec!["RED", "GREEN", "BLUE"]);
}

#[test]
fn end_to_end_when_multiple_enum_types_then_debug_has_all_defs() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    l : LEVEL;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();
    assert!(debug.enum_defs.iter().any(|e| e.type_name == "COLOR"));
    assert!(debug.enum_defs.iter().any(|e| e.type_name == "LEVEL"));
}
