//! End-to-end integration tests for ABS with LINT type.

#[macro_use]
mod common;

e2e_i64!(
    end_to_end_when_abs_lint_negative_then_positive,
    "PROGRAM main VAR x : LINT; y : LINT; END_VAR x := -7000000000; y := ABS(x); END_PROGRAM",
    &[(1, 7_000_000_000)],
);
