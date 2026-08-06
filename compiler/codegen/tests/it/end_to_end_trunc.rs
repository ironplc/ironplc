//! End-to-end integration tests for the TRUNC function.

e2e_i32!(
    end_to_end_when_trunc_real_positive_then_truncates_toward_zero,
    "
PROGRAM main
  VAR
    x : REAL;
    y : DINT;
  END_VAR
  x := 3.7;
  y := TRUNC(x);
END_PROGRAM
",
    &[(1, 3)],
);

e2e_i32!(
    end_to_end_when_trunc_real_negative_then_truncates_toward_zero,
    "
PROGRAM main
  VAR
    x : REAL;
    y : DINT;
  END_VAR
  x := -3.7;
  y := TRUNC(x);
END_PROGRAM
",
    &[(1, -3)],
);

e2e_i32!(
    end_to_end_when_trunc_real_zero_then_zero,
    "
PROGRAM main
  VAR
    x : REAL;
    y : DINT;
  END_VAR
  x := 0.0;
  y := TRUNC(x);
END_PROGRAM
",
    &[(1, 0)],
);

e2e_i64!(
    end_to_end_when_trunc_lreal_positive_then_truncates_toward_zero,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LINT;
  END_VAR
  x := 99.9;
  y := TRUNC(x);
END_PROGRAM
",
    &[(1, 99)],
);

e2e_i64!(
    end_to_end_when_trunc_lreal_negative_then_truncates_toward_zero,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LINT;
  END_VAR
  x := -99.9;
  y := TRUNC(x);
END_PROGRAM
",
    &[(1, -99)],
);
