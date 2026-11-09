//! Round-tripping of an array initializer written after the name of an
//! array type, `x : Triple := [1, 2, 3]`, in each position that takes one.

use super::common::*;

/// The array types the tests declare their variables with.
const TYPES: &str = "
TYPE
    Triple : ARRAY[1..3] OF DINT;
    Color : (Red, Green);
    Colors : ARRAY[1..2] OF Color;
END_TYPE
";

/// Round-trips `TYPES` followed by `source`.
fn assert_round_trips_with_types(source: &str) {
    assert_round_trips(&format!("{TYPES}{source}"), &CompilerOptions::default());
}

#[test]
fn write_to_string_when_pou_variables_of_named_array_have_values_then_round_trips() {
    assert_round_trips_with_types(
        "
PROGRAM main
VAR_INPUT
    i : Triple := [2(0), 1];
END_VAR
VAR_OUTPUT
    o : Triple := [1, 1, 1];
END_VAR
VAR
    x : Triple := [1, 2, 3];
    c : Colors := [Red, Green];
END_VAR
VAR_TEMP
    t : Triple := [3(7)];
END_VAR
END_PROGRAM
FUNCTION_BLOCK fb
VAR_INPUT
    i : Triple := [4, 5, 6];
END_VAR
END_FUNCTION_BLOCK
FUNCTION f : DINT
VAR
    v : Triple := [10, 20, 30];
END_VAR
f := v[1];
END_FUNCTION
",
    );
}

#[test]
fn write_to_string_when_located_variable_of_named_array_has_values_then_round_trips() {
    assert_round_trips_with_types(
        "
PROGRAM main
VAR
    x AT %MD10 : Triple := [4, 5, 6];
END_VAR
END_PROGRAM
",
    );
}

#[test]
fn write_to_string_when_structure_field_of_named_array_has_values_then_round_trips() {
    assert_round_trips_with_types(
        "
TYPE
    S : STRUCT
        f : Triple := [1, 2, 3];
    END_STRUCT;
END_TYPE
",
    );
}

#[test]
fn write_to_string_when_configuration_variables_of_named_array_have_values_then_round_trips() {
    assert_round_trips_with_types(
        "
PROGRAM main
VAR
    x : Triple;
END_VAR
END_PROGRAM
CONFIGURATION config
VAR_GLOBAL
    g : Triple := [4, 5, 6];
END_VAR
RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
END_RESOURCE
VAR_CONFIG
    res.plc_task_instance.x : Triple := [7, 8, 9];
END_VAR
END_CONFIGURATION
",
    );
}
