//! The `--allow-persistent-var` extension: the `PERSISTENT` variable
//! qualifier, alone or combined with `RETAIN`.

use super::common::*;
use rstest::rstest;

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

#[rstest]
#[case::input_persistent("VAR_INPUT PERSISTENT")]
#[case::output_persistent("VAR_OUTPUT PERSISTENT")]
#[case::input_retain_persistent("VAR_INPUT RETAIN PERSISTENT")]
#[case::output_retain_persistent("VAR_OUTPUT RETAIN PERSISTENT")]
#[case::var_retain_persistent("VAR RETAIN PERSISTENT")]
// Renders as `RETAIN PERSISTENT`: the two orders are one qualifier.
#[case::var_persistent_retain("VAR PERSISTENT RETAIN")]
fn write_to_string_when_function_block_block_qualified_then_round_trips(#[case] header: &str) {
    let source = format!(
        "
FUNCTION_BLOCK FB_Example
{header}
    nCounter : DINT;
END_VAR
END_FUNCTION_BLOCK
"
    );
    assert_round_trips(&source, &persistent_options());
}

#[test]
fn write_to_string_when_var_global_persistent_retain_then_round_trips() {
    let source = "
VAR_GLOBAL PERSISTENT RETAIN
    nCounter : DINT;
END_VAR
";
    assert_round_trips(source, &persistent_options());
}
