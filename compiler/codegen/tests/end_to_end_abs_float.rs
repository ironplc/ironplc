//! End-to-end integration tests for the ABS function with float types.

#[macro_use]
mod common;

e2e_f32_near!(
    end_to_end_when_abs_real_positive_then_unchanged,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := 42.5; y := ABS(x); END_PROGRAM",
    &[(1, 42.5)],
);

e2e_f32_near!(
    end_to_end_when_abs_real_negative_then_positive,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := -7.25; y := ABS(x); END_PROGRAM",
    &[(1, 7.25)],
);

e2e_f64_near!(
    end_to_end_when_abs_lreal_negative_then_positive,
    1e-12,
    "PROGRAM main VAR x : LREAL; y : LREAL; END_VAR x := -3.141592653589793; y := ABS(x); END_PROGRAM",
    &[(1, std::f64::consts::PI)],
);
