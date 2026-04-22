//! End-to-end integration tests for the LIMIT function with float types.

#[macro_use]
mod common;

// REAL (f32) LIMIT cases: x in range / below / above.
e2e_f32_near!(
    end_to_end_when_limit_real_in_range_then_unchanged,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := 5.0; y := LIMIT(0.0, x, 10.0); END_PROGRAM",
    &[(1, 5.0)],
);

e2e_f32_near!(
    end_to_end_when_limit_real_below_min_then_clamped,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := -5.0; y := LIMIT(0.0, x, 10.0); END_PROGRAM",
    &[(1, 0.0)],
);

e2e_f32_near!(
    end_to_end_when_limit_real_above_max_then_clamped,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := 99.0; y := LIMIT(0.0, x, 10.0); END_PROGRAM",
    &[(1, 10.0)],
);

e2e_f64_near!(
    end_to_end_when_limit_lreal_below_min_then_clamped,
    1e-12,
    "PROGRAM main VAR x : LREAL; y : LREAL; END_VAR x := -5.0; y := LIMIT(0.0, x, 10.0); END_PROGRAM",
    &[(1, 0.0)],
);
