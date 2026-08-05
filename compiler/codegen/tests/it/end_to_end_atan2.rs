//! End-to-end integration tests for ATAN2 function.

e2e_f32_near!(
    end_to_end_when_atan2_real_one_one_then_pi_quarter,
    1e-5,
    "PROGRAM main VAR y : REAL; x : REAL; result : REAL; END_VAR y := 1.0; x := 1.0; result := ATAN2(y, x); END_PROGRAM",
    &[(2, std::f32::consts::FRAC_PI_4)],
);

e2e_f32_near!(
    end_to_end_when_atan2_real_zero_one_then_zero,
    1e-5,
    "PROGRAM main VAR y : REAL; x : REAL; result : REAL; END_VAR y := 0.0; x := 1.0; result := ATAN2(y, x); END_PROGRAM",
    &[(2, 0.0)],
);

e2e_f32_near!(
    end_to_end_when_atan2_real_one_zero_then_pi_half,
    1e-5,
    "PROGRAM main VAR y : REAL; x : REAL; result : REAL; END_VAR y := 1.0; x := 0.0; result := ATAN2(y, x); END_PROGRAM",
    &[(2, std::f32::consts::FRAC_PI_2)],
);

e2e_f64_near!(
    end_to_end_when_atan2_lreal_neg_one_neg_one_then_neg_three_pi_quarter,
    1e-12,
    "PROGRAM main VAR y : LREAL; x : LREAL; result : LREAL; END_VAR y := -1.0; x := -1.0; result := ATAN2(y, x); END_PROGRAM",
    &[(2, -3.0 * std::f64::consts::FRAC_PI_4)],
);
