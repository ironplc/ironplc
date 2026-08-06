//! End-to-end integration tests for the POW/EXPT operator.

// 3^4 = 81
e2e_i32!(
    end_to_end_when_pow_expression_then_variable_has_power,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 3;
  y := x ** 4;
END_PROGRAM
",
    &[(0, 3), (1, 81)],
);

// 7^0 = 1
e2e_i32!(
    end_to_end_when_pow_with_zero_exponent_then_one,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 7;
  y := x ** 0;
END_PROGRAM
",
    &[(0, 7), (1, 1)],
);
