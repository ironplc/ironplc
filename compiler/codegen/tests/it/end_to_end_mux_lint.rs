//! End-to-end integration tests for MUX with LINT type.

e2e_i64!(
    end_to_end_when_mux_lint_k0_then_returns_in0,
    "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := MUX(0, LINT#5000000000, LINT#10000000000);
END_PROGRAM
",
    &[(0, 5_000_000_000)],
);

e2e_i64!(
    end_to_end_when_mux_lint_k1_then_returns_in1,
    "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := MUX(1, LINT#5000000000, LINT#10000000000);
END_PROGRAM
",
    &[(0, 10_000_000_000)],
);

e2e_i64!(
    end_to_end_when_mux_lint_k2_3_inputs_then_returns_in2,
    "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := MUX(2, LINT#100, LINT#200, LINT#300);
END_PROGRAM
",
    &[(0, 300)],
);

e2e_i64!(
    end_to_end_when_mux_lint_k_out_of_range_then_clamps_to_last,
    "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := MUX(10, LINT#100, LINT#200, LINT#300);
END_PROGRAM
",
    &[(0, 300)],
);

e2e_i64!(
    end_to_end_when_mux_lint_k_negative_then_clamps_to_first,
    "
PROGRAM main
  VAR
    k : DINT;
    result : LINT;
  END_VAR
  k := -1;
  result := MUX(k, LINT#100, LINT#200);
END_PROGRAM
",
    &[(1, 100)],
);
