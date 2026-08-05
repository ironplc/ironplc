//! End-to-end integration tests for the MAX function.

e2e_i32!(
    end_to_end_when_max_then_returns_larger,
    "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MAX(10, 3);
END_PROGRAM
",
    &[(0, 10)],
);

e2e_i32!(
    end_to_end_when_max_with_variable_then_returns_larger,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := MAX(x, 100);
END_PROGRAM
",
    &[(0, 5), (1, 100)],
);
