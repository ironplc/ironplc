//! End-to-end tests for real-to-integer type conversions.

#[macro_use]
mod common;

// Each case declares `x : <from>; y : <to>;`, sets a float value on x, then
// calls the conversion function into y. vars[1] holds the truncated integer.
e2e_i32!(
    end_to_end_when_real_to_int_then_truncates,
    "PROGRAM main VAR x : REAL; y : INT; END_VAR x := 3.14; y := REAL_TO_INT(x); END_PROGRAM",
    &[(1, 3)],
);

e2e_i32!(
    end_to_end_when_real_to_dint_negative_then_truncates,
    "PROGRAM main VAR x : REAL; y : DINT; END_VAR x := -7.9; y := REAL_TO_DINT(x); END_PROGRAM",
    &[(1, -7)],
);

e2e_i64!(
    end_to_end_when_lreal_to_lint_then_truncates,
    "PROGRAM main VAR x : LREAL; y : LINT; END_VAR x := 99.9; y := LREAL_TO_LINT(x); END_PROGRAM",
    &[(1, 99)],
);

e2e_i32!(
    end_to_end_when_real_to_sint_then_truncates_to_range,
    "PROGRAM main VAR x : REAL; y : SINT; END_VAR x := 50.7; y := REAL_TO_SINT(x); END_PROGRAM",
    &[(1, 50)],
);

e2e_i32!(
    end_to_end_when_lreal_to_udint_then_correct,
    "PROGRAM main VAR x : LREAL; y : UDINT; END_VAR x := 1000.0; y := LREAL_TO_UDINT(x); END_PROGRAM",
    &[(1, 1000)],
);
