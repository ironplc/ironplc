//! End-to-end integration tests for the MAX function with float types.

e2e_f32_near!(
    end_to_end_when_max_real_then_returns_larger,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 3.0;
  y := MAX(x, 7.5);
END_PROGRAM
",
    &[(1, 7.5)],
);

e2e_f32_near!(
    end_to_end_when_max_real_first_larger_then_returns_first,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 8.0;
  y := MAX(x, 2.0);
END_PROGRAM
",
    &[(1, 8.0)],
);

e2e_f64_near!(
    end_to_end_when_max_lreal_then_returns_larger,
    1e-12,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 3.0;
  y := MAX(x, 7.5);
END_PROGRAM
",
    &[(1, 7.5)],
);
