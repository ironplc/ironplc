//! End-to-end integration tests for the MIN function with float types.

e2e_f32_near!(
    end_to_end_when_min_real_then_returns_smaller,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := 7.5; y := MIN(x, 3.0); END_PROGRAM",
    &[(1, 3.0)],
);

e2e_f32_near!(
    end_to_end_when_min_real_first_smaller_then_returns_first,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := 2.0; y := MIN(x, 8.0); END_PROGRAM",
    &[(1, 2.0)],
);

e2e_f64_near!(
    end_to_end_when_min_lreal_then_returns_smaller,
    1e-12,
    "PROGRAM main VAR x : LREAL; y : LREAL; END_VAR x := 7.5; y := MIN(x, 3.0); END_PROGRAM",
    &[(1, 3.0)],
);
