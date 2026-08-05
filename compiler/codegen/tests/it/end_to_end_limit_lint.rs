//! End-to-end integration tests for LIMIT with LINT type.

e2e_i64!(
    end_to_end_when_limit_lint_in_range_then_unchanged,
    "PROGRAM main VAR result : LINT; END_VAR result := LIMIT(LINT#-10000000000, LINT#5000000000, LINT#10000000000); END_PROGRAM",
    &[(0, 5_000_000_000)],
);

e2e_i64!(
    end_to_end_when_limit_lint_below_min_then_clamped,
    "PROGRAM main VAR result : LINT; END_VAR result := LIMIT(LINT#0, LINT#-5000000000, LINT#10000000000); END_PROGRAM",
    &[(0, 0)],
);
