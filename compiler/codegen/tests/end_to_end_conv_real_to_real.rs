//! End-to-end tests for real-to-real type conversions.

#[macro_use]
mod common;

e2e_f64_near!(
    end_to_end_when_real_to_lreal_then_widens,
    0.01,
    "PROGRAM main VAR x : REAL; y : LREAL; END_VAR x := 1.5; y := REAL_TO_LREAL(x); END_PROGRAM",
    &[(1, 1.5)],
);

e2e_f32_near!(
    end_to_end_when_lreal_to_real_then_narrows,
    1e-4,
    "PROGRAM main VAR x : LREAL; y : REAL; END_VAR x := 9.876543210; y := LREAL_TO_REAL(x); END_PROGRAM",
    &[(1, 9.876543)],
);
