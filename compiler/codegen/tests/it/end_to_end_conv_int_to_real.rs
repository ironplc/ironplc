//! End-to-end tests for integer-to-real type conversions.

e2e_f32_near!(
    end_to_end_when_int_to_real_then_correct,
    1e-5,
    "
PROGRAM main
  VAR
    x : INT;
    y : REAL;
  END_VAR
  x := 42;
  y := INT_TO_REAL(x);
END_PROGRAM
",
    &[(1, 42.0)],
);

e2e_f64_near!(
    end_to_end_when_dint_to_lreal_then_correct,
    1e-12,
    "
PROGRAM main
  VAR
    x : DINT;
    y : LREAL;
  END_VAR
  x := -100;
  y := DINT_TO_LREAL(x);
END_PROGRAM
",
    &[(1, -100.0)],
);

e2e_f32_near!(
    end_to_end_when_sint_to_real_then_correct,
    1e-5,
    "
PROGRAM main
  VAR
    x : SINT;
    y : REAL;
  END_VAR
  x := -7;
  y := SINT_TO_REAL(x);
END_PROGRAM
",
    &[(1, -7.0)],
);

e2e_f64_near!(
    end_to_end_when_lint_to_lreal_then_correct,
    1.0,
    "
PROGRAM main
  VAR
    x : LINT;
    y : LREAL;
  END_VAR
  x := 123456789;
  y := LINT_TO_LREAL(x);
END_PROGRAM
",
    &[(1, 123456789.0)],
);

e2e_f32_near!(
    end_to_end_when_uint_to_real_then_correct,
    1.0,
    "
PROGRAM main
  VAR
    x : UINT;
    y : REAL;
  END_VAR
  x := 40000;
  y := UINT_TO_REAL(x);
END_PROGRAM
",
    &[(1, 40000.0)],
);
