//! End-to-end integration tests for the arithmetic-identity peephole.
//!
//! The sign of a float zero is invisible to `assert_eq!` (`-0.0 == 0.0`),
//! so the REAL/LREAL tests divide by the result: `1.0 / (+0.0)` is `+inf`
//! and `1.0 / (-0.0)` is `-inf`. IEEE 754 says `(-0.0) + 0.0 = +0.0`, so
//! the optimizer must not remove the add; `(-0.0) - 0.0 = -0.0`, so it may
//! remove the subtract.

e2e_f32!(
    end_to_end_when_real_negative_zero_plus_zero_then_positive_zero,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    z : REAL;
  END_VAR
  x := -0.0;
  y := x + 0.0;
  z := 1.0 / y;
END_PROGRAM
",
    &[(2, f32::INFINITY)],
);

e2e_f64!(
    end_to_end_when_lreal_negative_zero_plus_zero_then_positive_zero,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
    z : LREAL;
  END_VAR
  x := -0.0;
  y := x + 0.0;
  z := 1.0 / y;
END_PROGRAM
",
    &[(2, f64::INFINITY)],
);

e2e_f32!(
    end_to_end_when_real_negative_zero_minus_zero_then_negative_zero,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    z : REAL;
  END_VAR
  x := -0.0;
  y := x - 0.0;
  z := 1.0 / y;
END_PROGRAM
",
    &[(2, f32::NEG_INFINITY)],
);

e2e_f64!(
    end_to_end_when_lreal_negative_zero_minus_zero_then_negative_zero,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
    z : LREAL;
  END_VAR
  x := -0.0;
  y := x - 0.0;
  z := 1.0 / y;
END_PROGRAM
",
    &[(2, f64::NEG_INFINITY)],
);

e2e_f32!(
    end_to_end_when_real_plus_zero_then_unchanged,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 2.5;
  y := x + 0.0;
END_PROGRAM
",
    &[(1, 2.5)],
);

e2e_f64!(
    end_to_end_when_lreal_plus_zero_then_unchanged,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 2.5;
  y := x + 0.0;
END_PROGRAM
",
    &[(1, 2.5)],
);

e2e_i32!(
    end_to_end_when_dint_plus_zero_then_unchanged,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := -7;
  y := x + 0;
END_PROGRAM
",
    &[(0, -7), (1, -7)],
);

e2e_i32!(
    end_to_end_when_dint_minus_zero_then_unchanged,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := -7;
  y := x - 0;
END_PROGRAM
",
    &[(0, -7), (1, -7)],
);

e2e_i64!(
    end_to_end_when_lint_plus_zero_then_unchanged,
    "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := -7;
  y := x + 0;
END_PROGRAM
",
    &[(0, -7), (1, -7)],
);
