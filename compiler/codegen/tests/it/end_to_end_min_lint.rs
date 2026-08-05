//! End-to-end integration tests for MIN with LINT type.

e2e_i64!(
    end_to_end_when_min_lint_then_returns_smaller,
    "PROGRAM main VAR a : LINT; b : LINT; result : LINT; END_VAR a := -5000000000; b := 3000000000; result := MIN(a, b); END_PROGRAM",
    &[(2, -5_000_000_000)],
);
