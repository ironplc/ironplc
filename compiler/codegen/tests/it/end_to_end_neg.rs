//! End-to-end integration tests for the NEG unary operator.

e2e_i32!(
    end_to_end_when_neg_variable_then_negated,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 7; y := -x; END_PROGRAM",
    &[(0, 7), (1, -7)],
);

e2e_i32!(
    end_to_end_when_neg_negative_variable_then_positive,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := -3; y := -x; END_PROGRAM",
    &[(0, -3), (1, 3)],
);

e2e_i32!(
    end_to_end_when_double_neg_then_original,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 42; y := -(-x); END_PROGRAM",
    &[(0, 42), (1, 42)],
);
