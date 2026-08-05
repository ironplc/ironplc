//! End-to-end integration tests for the SEL function.

e2e_i32!(
    end_to_end_when_sel_false_then_returns_in0,
    "PROGRAM main VAR y : DINT; END_VAR y := SEL(0, 10, 20); END_PROGRAM",
    &[(0, 10)],
);

e2e_i32!(
    end_to_end_when_sel_true_then_returns_in1,
    "PROGRAM main VAR y : DINT; END_VAR y := SEL(1, 10, 20); END_PROGRAM",
    &[(0, 20)],
);

e2e_i32!(
    end_to_end_when_sel_with_variable_then_selects,
    "PROGRAM main VAR g : DINT; y : DINT; END_VAR g := 1; y := SEL(g, 100, 200); END_PROGRAM",
    &[(0, 1), (1, 200)],
);
