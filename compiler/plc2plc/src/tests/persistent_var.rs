//! Beckhoff TwinCAT/CODESYS `PERSISTENT` variable qualifier.

use super::common::*;

fn persistent_options() -> CompilerOptions {
    CompilerOptions {
        allow_persistent_var: true,
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn write_to_string_when_var_global_persistent_then_round_trips() {
    let source = "
VAR_GLOBAL PERSISTENT
    nCounter : DINT;
END_VAR
";
    assert_round_trips(source, &persistent_options());
}

#[test]
fn write_to_string_when_program_var_persistent_then_round_trips() {
    let source = "
PROGRAM main
VAR PERSISTENT
    nCounter : DINT;
END_VAR
END_PROGRAM
";
    assert_round_trips(source, &persistent_options());
}

#[test]
fn write_to_string_when_function_block_var_persistent_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR PERSISTENT
    nCounter : DINT;
END_VAR
END_FUNCTION_BLOCK
";
    assert_round_trips(source, &persistent_options());
}
