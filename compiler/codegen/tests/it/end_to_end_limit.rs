//! End-to-end integration tests for the LIMIT function.

e2e_i32!(
    end_to_end_when_limit_in_range_then_unchanged,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 5; y := LIMIT(0, x, 10); END_PROGRAM",
    &[(0, 5), (1, 5)],
);

e2e_i32!(
    end_to_end_when_limit_below_min_then_clamped,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := -5; y := LIMIT(0, x, 10); END_PROGRAM",
    &[(0, -5), (1, 0)],
);

e2e_i32!(
    end_to_end_when_limit_above_max_then_clamped,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 15; y := LIMIT(0, x, 10); END_PROGRAM",
    &[(0, 15), (1, 10)],
);
