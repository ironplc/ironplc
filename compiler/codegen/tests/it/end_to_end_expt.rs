//! End-to-end integration tests for EXPT with DINT type.

e2e_i32!(
    end_to_end_when_expt_then_correct_result,
    "PROGRAM main VAR result : DINT; END_VAR result := EXPT(2, 10); END_PROGRAM",
    &[(0, 1024)],
);

e2e_i32!(
    end_to_end_when_expt_zero_exponent_then_one,
    "PROGRAM main VAR result : DINT; END_VAR result := EXPT(5, 0); END_PROGRAM",
    &[(0, 1)],
);
