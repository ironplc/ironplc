//! End-to-end integration tests for MAX with LINT type.

e2e_i64!(
    end_to_end_when_max_lint_first_larger_then_returns_first,
    "PROGRAM main VAR a : LINT; b : LINT; result : LINT; END_VAR a := 10000000000; b := 5000000000; result := MAX(a, b); END_PROGRAM",
    &[(2, 10_000_000_000)],
);
