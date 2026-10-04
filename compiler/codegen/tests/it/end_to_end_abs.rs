//! End-to-end integration tests for the ABS function.

e2e_i32!(
    end_to_end_when_abs_positive_then_unchanged,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 42;
  y := ABS(x);
END_PROGRAM
",
    &[("x", 42), ("y", 42)],
);

e2e_i32!(
    end_to_end_when_abs_negative_then_positive,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := -7;
  y := ABS(x);
END_PROGRAM
",
    &[("x", -7), ("y", 7)],
);
