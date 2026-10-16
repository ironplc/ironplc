//! End-to-end integration tests for EXPT with DINT type.

e2e_i32!(
    end_to_end_when_expt_then_correct_result,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := EXPT(2, 10);
END_PROGRAM
",
    &[("result", 1024)],
);

e2e_i32!(
    end_to_end_when_expt_zero_exponent_then_one,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := EXPT(5, 0);
END_PROGRAM
",
    &[("result", 1)],
);

// An unsigned exponent is never negative: a UDINT exponent past the range of
// DINT wraps the power to 32 bits rather than trapping.
e2e_i64!(
    end_to_end_when_expt_of_udint_exponent_past_dint_then_wraps,
    "
PROGRAM main
  VAR
    base : UDINT := 3;
    exponent : UDINT := 3000000000;
    result : UDINT;
  END_VAR
  result := EXPT(base, exponent);
END_PROGRAM
",
    &[("result", 3_840_514_049)],
);

// As for UDINT, a ULINT exponent past the range of LINT wraps the power to 64
// bits.
e2e_i64!(
    end_to_end_when_expt_of_ulint_exponent_past_lint_then_wraps,
    "
PROGRAM main
  VAR
    base : ULINT := 3;
    exponent : ULINT := 9223372036854775809;
    result : ULINT;
  END_VAR
  result := base ** exponent;
END_PROGRAM
",
    &[("result", 3)],
);
