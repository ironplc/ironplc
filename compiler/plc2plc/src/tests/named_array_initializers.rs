//! Round-tripping of an array initializer written after the name of an
//! array type, `x : A3 := [1, 2, 3]`, in each position that takes one.

use super::common::*;

#[test]
fn write_to_string_when_pou_variables_of_named_array_have_values_then_round_trips() {
    let source = "
PROGRAM main
VAR_INPUT
    i : A3 := [2(0), 1];
END_VAR
VAR_OUTPUT
    o : A3 := [1, 1, 1];
END_VAR
VAR
    x : A3 := [1, 2, 3];
    c : COLORS := [Red, Green];
END_VAR
VAR_TEMP
    t : A3 := [3(7)];
END_VAR
END_PROGRAM
FUNCTION_BLOCK fb
VAR_INPUT
    i : A3 := [4, 5, 6];
END_VAR
END_FUNCTION_BLOCK
FUNCTION f : DINT
VAR
    v : A3 := [10, 20, 30];
END_VAR
f := v[1];
END_FUNCTION
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_located_variable_of_named_array_has_values_then_round_trips() {
    let source = "
PROGRAM main
VAR
    x AT %MD10 : A3 := [4, 5, 6];
END_VAR
END_PROGRAM
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_structure_field_of_named_array_has_values_then_round_trips() {
    let source = "
TYPE
    S : STRUCT
        f : A3 := [1, 2, 3];
    END_STRUCT;
END_TYPE
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_configuration_variables_of_named_array_have_values_then_round_trips() {
    let source = "
CONFIGURATION config
VAR_GLOBAL
    g : A3 := [4, 5, 6];
END_VAR
RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
END_RESOURCE
VAR_CONFIG
    res.plc_task_instance.x : A3 := [7, 8, 9];
END_VAR
END_CONFIGURATION
";
    assert_round_trips(source, &CompilerOptions::default());
}
