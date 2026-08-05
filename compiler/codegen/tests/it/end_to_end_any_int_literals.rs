//! End-to-end integration tests for bare literal type inference (ANY_INT / ANY_REAL).
//!
//! Bare integer literals (e.g. `5`) resolve as ANY_INT and are compatible with
//! any integer parameter type. Bare real literals resolve as ANY_REAL and are
//! compatible with any real parameter type.

e2e_i32!(
    end_to_end_when_bare_int_literal_to_int_param_then_correct,
    "FUNCTION ADD_ONE : INT VAR_INPUT x : INT; END_VAR ADD_ONE := x + INT#1; END_FUNCTION PROGRAM main VAR result : INT; END_VAR result := ADD_ONE(5); END_PROGRAM",
    &[(0, 6)],
);

e2e_i32!(
    end_to_end_when_bare_int_literal_to_sint_param_then_correct,
    "FUNCTION DOUBLE : SINT VAR_INPUT x : SINT; END_VAR DOUBLE := x + x; END_FUNCTION PROGRAM main VAR result : SINT; END_VAR result := DOUBLE(7); END_PROGRAM",
    &[(0, 14)],
);

e2e_i32!(
    end_to_end_when_bare_int_literal_to_dint_param_then_correct,
    "FUNCTION TRIPLE : DINT VAR_INPUT x : DINT; END_VAR TRIPLE := x + x + x; END_FUNCTION PROGRAM main VAR result : DINT; END_VAR result := TRIPLE(100); END_PROGRAM",
    &[(0, 300)],
);

e2e_f64_near!(
    end_to_end_when_bare_real_literal_to_lreal_param_then_correct,
    0.001,
    "FUNCTION ADD_PI : LREAL VAR_INPUT x : LREAL; END_VAR ADD_PI := x; END_FUNCTION PROGRAM main VAR result : LREAL; END_VAR result := ADD_PI(3.25); END_PROGRAM",
    &[(0, 3.25)],
);

e2e_f32_near!(
    end_to_end_when_bare_int_literal_to_real_param_then_correct,
    1e-5,
    "FUNCTION HALVE : REAL VAR_INPUT x : REAL; END_VAR HALVE := x / REAL#2.0; END_FUNCTION PROGRAM main VAR result : REAL; END_VAR result := HALVE(10); END_PROGRAM",
    &[(0, 5.0)],
);

e2e_f64_near!(
    end_to_end_when_bare_int_literal_to_lreal_param_then_correct,
    1e-10,
    "FUNCTION IDENTITY_LREAL : LREAL VAR_INPUT x : LREAL; END_VAR IDENTITY_LREAL := x; END_FUNCTION PROGRAM main VAR result : LREAL; END_VAR result := IDENTITY_LREAL(42); END_PROGRAM",
    &[(0, 42.0)],
);

e2e_i32!(
    end_to_end_when_bare_literal_in_expression_with_int_var_then_correct,
    "PROGRAM main VAR x : INT; result : INT; END_VAR x := INT#10; result := x + 5; END_PROGRAM",
    &[(1, 15)],
);
