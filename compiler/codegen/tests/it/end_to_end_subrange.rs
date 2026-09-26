//! End-to-end integration tests for subrange type compilation.

// Default value for subrange is the lower bound (1)
e2e_i32!(
    end_to_end_when_subrange_var_no_init_then_default_is_lower_bound,
    "
TYPE
  MY_RANGE : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE;
  END_VAR
END_PROGRAM
",
    &[(0, 1)],
);

e2e_i32!(
    end_to_end_when_subrange_var_with_init_then_uses_init_value,
    "
TYPE
  MY_RANGE : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE := 75;
  END_VAR
END_PROGRAM
",
    &[(0, 75)],
);

e2e_i32!(
    end_to_end_when_subrange_var_assigned_then_stores_value,
    "
TYPE
  MY_RANGE : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE;
  END_VAR
  x := 42;
END_PROGRAM
",
    &[(0, 42)],
);

e2e_i32!(
    end_to_end_when_subrange_var_in_expression_then_computes,
    "
TYPE
  MY_RANGE : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE;
    y : DINT;
  END_VAR
  x := 10;
  y := x + 5;
END_PROGRAM
",
    &[(0, 10), (1, 15)],
);

// Alias inherits the lower bound from the base subrange type
e2e_i32!(
    end_to_end_when_subrange_alias_var_then_default_is_lower_bound,
    "
TYPE
  BASE_RANGE : INT (1..100);
  ALIAS_RANGE : BASE_RANGE;
END_TYPE

PROGRAM main
  VAR
    x : ALIAS_RANGE;
  END_VAR
END_PROGRAM
",
    &[(0, 1)],
);

// Nested alias resolves to the original subrange; default = 10
e2e_i32!(
    end_to_end_when_nested_subrange_alias_var_then_works,
    "
TYPE
  BASE_RANGE : INT (10..50);
  MID_RANGE : BASE_RANGE;
  TOP_RANGE : MID_RANGE;
END_TYPE

PROGRAM main
  VAR
    x : TOP_RANGE;
  END_VAR
END_PROGRAM
",
    &[(0, 10)],
);

e2e_i32!(
    end_to_end_when_subrange_alias_with_init_then_uses_init,
    "
TYPE
  BASE_RANGE : INT (1..100);
  ALIAS_RANGE : BASE_RANGE;
END_TYPE

PROGRAM main
  VAR
    x : ALIAS_RANGE := 42;
  END_VAR
END_PROGRAM
",
    &[(0, 42)],
);

// Default value for unsigned subrange is the lower bound (10)
e2e_i32!(
    end_to_end_when_subrange_unsigned_base_then_works,
    "
TYPE
  U_RANGE : UINT (10..200);
END_TYPE

PROGRAM main
  VAR
    x : U_RANGE;
  END_VAR
END_PROGRAM
",
    &[(0, 10)],
);

// A subrange of a 64-bit base compares at 64 bits: the literal fits ULINT,
// and x (5_000_000_000) is above it.
e2e_i64!(
    end_to_end_when_ulint_subrange_compared_then_uses_base_width,
    "
TYPE
  BIG_RANGE : ULINT (0..10000000000);
END_TYPE

PROGRAM main
  VAR
    x : BIG_RANGE := 5000000000;
    d : LINT;
  END_VAR
  IF x > 4000000000 THEN d := 1; ELSE d := 2; END_IF;
END_PROGRAM
",
    &[(1, 1)],
);

// A CASE selector of a 64-bit subrange matches a label above the i32 range.
e2e_i64!(
    end_to_end_when_ulint_subrange_case_selector_then_matches_wide_label,
    "
TYPE
  BIG_RANGE : ULINT (0..10000000000);
END_TYPE

PROGRAM main
  VAR
    x : BIG_RANGE := 5000000000;
    d : LINT;
  END_VAR
  CASE x OF
    5000000000: d := 1;
  ELSE
    d := 2;
  END_CASE;
END_PROGRAM
",
    &[(1, 1)],
);
