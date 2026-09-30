//! Direct addresses with multi-digit fields (`%MW10`, `%IX12.7`).

use super::common::*;

#[test]
fn write_to_string_when_located_vars_have_multi_digit_fields_then_round_trips() {
    let source = "
PROGRAM main
VAR
    speed_setpoint AT %MW10 : INT;
    limit AT %IX12.7 : BOOL;
    total AT %QD100 : DINT;
    deep AT %IX1.2.3.4 : BOOL;
    plain AT %I10 : BOOL;
END_VAR
END_PROGRAM
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_direct_variable_in_expression_then_round_trips() {
    let source = "
PROGRAM main
VAR
    x : INT;
END_VAR
    x := %IW10;
END_PROGRAM
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_address_has_underscore_and_lower_case_then_renders_canonical() {
    // The AST keeps the field value, not its spelling, so the rendering is
    // the canonical upper-case spelling without the digit separator.
    let source = "
PROGRAM main
VAR
    x AT %mw1_000 : INT;
END_VAR
END_PROGRAM
";
    let rendered = assert_round_trips(source, &CompilerOptions::default());

    assert!(rendered.contains("%MW1000"), "{rendered}");
}
