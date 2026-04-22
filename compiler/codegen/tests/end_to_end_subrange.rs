//! End-to-end integration tests for subrange type compilation.

#[macro_use]
mod common;

// Subrange var default = lower bound (1).
e2e_i32!(
    end_to_end_when_subrange_var_no_init_then_default_is_lower_bound,
    "TYPE MY_RANGE : INT (1..100); END_TYPE PROGRAM main VAR x : MY_RANGE; END_VAR END_PROGRAM",
    &[(0, 1)],
);

// Explicit init value overrides the lower-bound default.
e2e_i32!(
    end_to_end_when_subrange_var_with_init_then_uses_init_value,
    "TYPE MY_RANGE : INT (1..100); END_TYPE PROGRAM main VAR x : MY_RANGE := 75; END_VAR END_PROGRAM",
    &[(0, 75)],
);

e2e_i32!(
    end_to_end_when_subrange_var_assigned_then_stores_value,
    "TYPE MY_RANGE : INT (1..100); END_TYPE PROGRAM main VAR x : MY_RANGE; END_VAR x := 42; END_PROGRAM",
    &[(0, 42)],
);

// Subrange var participates in arithmetic with a DINT target.
e2e_i32!(
    end_to_end_when_subrange_var_in_expression_then_computes,
    "TYPE MY_RANGE : INT (1..100); END_TYPE PROGRAM main VAR x : MY_RANGE; y : DINT; END_VAR x := 10; y := x + 5; END_PROGRAM",
    &[(0, 10), (1, 15)],
);

// Single-level alias: default of aliased subrange = lower bound (1).
e2e_i32!(
    end_to_end_when_subrange_alias_var_then_default_is_lower_bound,
    "TYPE BASE_RANGE : INT (1..100); ALIAS_RANGE : BASE_RANGE; END_TYPE PROGRAM main VAR x : ALIAS_RANGE; END_VAR END_PROGRAM",
    &[(0, 1)],
);

// Two-level alias resolves to the original subrange; default = 10.
e2e_i32!(
    end_to_end_when_nested_subrange_alias_var_then_works,
    "TYPE BASE_RANGE : INT (10..50); MID_RANGE : BASE_RANGE; TOP_RANGE : MID_RANGE; END_TYPE PROGRAM main VAR x : TOP_RANGE; END_VAR END_PROGRAM",
    &[(0, 10)],
);

// Alias with explicit init value.
e2e_i32!(
    end_to_end_when_subrange_alias_with_init_then_uses_init,
    "TYPE BASE_RANGE : INT (1..100); ALIAS_RANGE : BASE_RANGE; END_TYPE PROGRAM main VAR x : ALIAS_RANGE := 42; END_VAR END_PROGRAM",
    &[(0, 42)],
);

// Unsigned-base subrange: default = lower bound (10).
e2e_i32!(
    end_to_end_when_subrange_unsigned_base_then_works,
    "TYPE U_RANGE : UINT (10..200); END_TYPE PROGRAM main VAR x : U_RANGE; END_VAR END_PROGRAM",
    &[(0, 10)],
);
