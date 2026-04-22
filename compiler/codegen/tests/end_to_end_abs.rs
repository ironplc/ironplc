//! End-to-end integration tests for the ABS function.

#[macro_use]
mod common;

e2e_i32!(
    end_to_end_when_abs_positive_then_unchanged,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 42; y := ABS(x); END_PROGRAM",
    &[(0, 42), (1, 42)],
);

e2e_i32!(
    end_to_end_when_abs_negative_then_positive,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := -7; y := ABS(x); END_PROGRAM",
    &[(0, -7), (1, 7)],
);
