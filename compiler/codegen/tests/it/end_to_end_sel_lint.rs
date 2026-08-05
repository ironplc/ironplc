//! End-to-end integration tests for SEL with LINT type.

e2e_i64!(
    end_to_end_when_sel_lint_false_then_returns_in0,
    "PROGRAM main VAR result : LINT; END_VAR result := SEL(0, LINT#5000000000, LINT#10000000000); END_PROGRAM",
    &[(0, 5_000_000_000)],
);

e2e_i64!(
    end_to_end_when_sel_lint_true_then_returns_in1,
    "PROGRAM main VAR result : LINT; END_VAR result := SEL(1, LINT#5000000000, LINT#10000000000); END_PROGRAM",
    &[(0, 10_000_000_000)],
);
