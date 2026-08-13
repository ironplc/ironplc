//! End-to-end integration tests for the MIN function.

e2e_i32!(
    end_to_end_when_min_then_returns_smaller,
    "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MIN(10, 3);
END_PROGRAM
",
    &[(0, 3)],
);

e2e_i32!(
    end_to_end_when_min_with_variable_then_returns_smaller,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := MIN(x, 100);
END_PROGRAM
",
    &[(0, 5), (1, 5)],
);
