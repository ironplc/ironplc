//! End-to-end integration tests for LN, LOG, EXP functions.

e2e_f32_near!(
    end_to_end_when_ln_real_then_correct,
    1e-4,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := 2.718282; y := LN(x); END_PROGRAM",
    &[(1, 1.0)],
);

e2e_f64_near!(
    end_to_end_when_ln_lreal_then_correct,
    1e-12,
    "PROGRAM main VAR x : LREAL; y : LREAL; END_VAR x := 1.0; y := LN(x); END_PROGRAM",
    &[(1, 0.0)],
);

e2e_f32_near!(
    end_to_end_when_log_real_then_correct,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := 100.0; y := LOG(x); END_PROGRAM",
    &[(1, 2.0)],
);

e2e_f64_near!(
    end_to_end_when_log_lreal_then_correct,
    1e-12,
    "PROGRAM main VAR x : LREAL; y : LREAL; END_VAR x := 1000.0; y := LOG(x); END_PROGRAM",
    &[(1, 3.0)],
);

e2e_f32_near!(
    end_to_end_when_exp_real_then_correct,
    1e-5,
    "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := 0.0; y := EXP(x); END_PROGRAM",
    &[(1, 1.0)],
);

e2e_f64_near!(
    end_to_end_when_exp_lreal_then_correct,
    1e-12,
    "PROGRAM main VAR x : LREAL; y : LREAL; END_VAR x := 1.0; y := EXP(x); END_PROGRAM",
    &[(1, std::f64::consts::E)],
);
