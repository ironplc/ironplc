//! End-to-end tests for the declared initial values of function block
//! fields (#1355, #1524): every instance starts with them, as a PROGRAM
//! variable does, and an instance initializer overrides them.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

/// Runs `source` for one scan and returns the value of the program variable
/// at `slot`.
fn run_i32(source: &str, slot: usize) -> i32 {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    bufs.vars[slot].as_i32()
}

#[test]
fn end_to_end_when_fb_local_has_initial_value_then_first_call_sees_it() {
    // #1355: 7 + 5.
    let source = "
FUNCTION_BLOCK FB_P
VAR_INPUT x : INT; END_VAR
VAR a : INT := 5; END_VAR
VAR_OUTPUT y : INT; END_VAR
y := x + a;
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : FB_P; result : INT; END_VAR
    inst(x := 7, y => result);
END_PROGRAM
";
    assert_eq!(run_i32(source, 1), 12);
}

#[test]
fn end_to_end_when_fb_field_read_from_outside_then_initial_value() {
    // #1524.
    let source = "
FUNCTION_BLOCK FB_Counter
VAR n : INT := 5; END_VAR
    n := n;
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : FB_Counter; observed : INT; END_VAR
    inst();
    observed := inst.n;
END_PROGRAM
";
    assert_eq!(run_i32(source, 1), 5);
}

#[test]
fn end_to_end_when_instance_initializer_then_overrides_type_default() {
    let source = "
FUNCTION_BLOCK FB_P
VAR a : INT := 5; END_VAR
VAR_OUTPUT y : INT; END_VAR
y := a;
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : FB_P := (a := 9); result : INT; END_VAR
    inst(y => result);
END_PROGRAM
";
    assert_eq!(run_i32(source, 1), 9);
}

#[test]
fn end_to_end_when_input_and_output_have_initial_values_then_used() {
    // An input not passed keeps its initial value; an output starts with it.
    let source = "
FUNCTION_BLOCK FB_IO
VAR_INPUT x : DINT := 40; END_VAR
VAR_OUTPUT y : DINT := 2; END_VAR
y := x + y;
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : FB_IO; result : DINT; END_VAR
    inst(y => result);
END_PROGRAM
";
    assert_eq!(run_i32(source, 1), 42);
}

#[test]
fn end_to_end_when_real_and_bool_fields_have_initial_values_then_used() {
    let source = "
FUNCTION_BLOCK FB_R
VAR r : REAL := 2.5; b : BOOL := TRUE; END_VAR
VAR_OUTPUT y : INT; END_VAR
IF b THEN
    y := REAL_TO_INT(r * 4.0);
END_IF;
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : FB_R; result : INT; END_VAR
    inst(y => result);
END_PROGRAM
";
    assert_eq!(run_i32(source, 1), 10);
}

#[test]
fn end_to_end_when_enum_field_has_initial_value_then_used() {
    let source = "
TYPE Color : (Red, Green, Blue); END_TYPE
FUNCTION_BLOCK FB_E
VAR c : Color := Blue; END_VAR
VAR_OUTPUT y : INT; END_VAR
IF c = Blue THEN
    y := 1;
END_IF;
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : FB_E; result : INT; END_VAR
    inst(y => result);
END_PROGRAM
";
    assert_eq!(run_i32(source, 1), 1);
}

#[test]
fn end_to_end_when_two_instances_then_each_starts_with_initial_value() {
    // The first instance's call changes its own copy only.
    let source = "
FUNCTION_BLOCK FB_C
VAR n : INT := 5; END_VAR
VAR_OUTPUT y : INT; END_VAR
n := n + 1;
y := n;
END_FUNCTION_BLOCK
PROGRAM main
VAR a : FB_C; b : FB_C; ra : INT; rb : INT; END_VAR
    a(y => ra);
    a(y => ra);
    b(y => rb);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i32(), 7);
    assert_eq!(bufs.vars[3].as_i32(), 6);
}
