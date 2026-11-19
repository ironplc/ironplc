//! A variable declared with a subrange spelled in place (`x : INT(0..15)`)
//! round-trips in each block whose grammar allows it.

use super::common::*;

#[test]
fn write_to_string_when_inline_subrange_variables_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB
VAR_INPUT
    i : INT(5..15);
END_VAR
VAR_OUTPUT
    o : INT(6..15) := 8;
END_VAR
VAR
    x, y : UINT(2..15) := 4;
    n : DINT(-10..-2) := -3;
    m AT %MW0 : INT(0..15);
END_VAR
VAR_TEMP
    t : INT(0..3);
END_VAR
VAR CONSTANT
    c : INT(0..15) := 3;
END_VAR
VAR_EXTERNAL
    g : INT(5..15);
END_VAR
END_FUNCTION_BLOCK

CONFIGURATION config
VAR_GLOBAL
    g : INT(5..15) := 9;
END_VAR
RESOURCE res ON PLC
    PROGRAM p : main;
END_RESOURCE
END_CONFIGURATION
";
    assert_round_trips(source, &CompilerOptions::default());
}
