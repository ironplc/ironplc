//! End-to-end integration tests for SIN, COS, TAN, ASIN, ACOS, ATAN functions.

e2e_f32_near!(
    end_to_end_when_sin_real_zero_then_zero,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := SIN(x);
END_PROGRAM
",
    &[(1, 0.0)],
);

e2e_f64_near!(
    end_to_end_when_sin_lreal_pi_half_then_one,
    1e-12,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.5707963267948966;
  y := SIN(x);
END_PROGRAM
",
    &[(1, 1.0)],
);

e2e_f32_near!(
    end_to_end_when_cos_real_zero_then_one,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := COS(x);
END_PROGRAM
",
    &[(1, 1.0)],
);

e2e_f64_near!(
    end_to_end_when_cos_lreal_pi_then_neg_one,
    1e-12,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 3.141592653589793;
  y := COS(x);
END_PROGRAM
",
    &[(1, -1.0)],
);

e2e_f32_near!(
    end_to_end_when_tan_real_zero_then_zero,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := TAN(x);
END_PROGRAM
",
    &[(1, 0.0)],
);

e2e_f64_near!(
    end_to_end_when_tan_lreal_pi_quarter_then_one,
    1e-12,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 0.7853981633974483;
  y := TAN(x);
END_PROGRAM
",
    &[(1, 1.0)],
);

e2e_f32_near!(
    end_to_end_when_asin_real_zero_then_zero,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := ASIN(x);
END_PROGRAM
",
    &[(1, 0.0)],
);

e2e_f64_near!(
    end_to_end_when_asin_lreal_one_then_pi_half,
    1e-12,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.0;
  y := ASIN(x);
END_PROGRAM
",
    &[(1, std::f64::consts::FRAC_PI_2)],
);

e2e_f32_near!(
    end_to_end_when_acos_real_one_then_zero,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 1.0;
  y := ACOS(x);
END_PROGRAM
",
    &[(1, 0.0)],
);

e2e_f64_near!(
    end_to_end_when_acos_lreal_zero_then_pi_half,
    1e-12,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 0.0;
  y := ACOS(x);
END_PROGRAM
",
    &[(1, std::f64::consts::FRAC_PI_2)],
);

e2e_f32_near!(
    end_to_end_when_atan_real_zero_then_zero,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := ATAN(x);
END_PROGRAM
",
    &[(1, 0.0)],
);

e2e_f64_near!(
    end_to_end_when_atan_lreal_one_then_pi_quarter,
    1e-12,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.0;
  y := ATAN(x);
END_PROGRAM
",
    &[(1, std::f64::consts::FRAC_PI_4)],
);
