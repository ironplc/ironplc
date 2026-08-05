//! End-to-end integration tests for EXPT with LINT type.

e2e_i64!(
    end_to_end_when_expt_lint_then_correct,
    "PROGRAM main VAR base : LINT; exp : LINT; result : LINT; END_VAR base := 2; exp := 40; result := EXPT(base, exp); END_PROGRAM",
    &[(2, 1_099_511_627_776)],
);
