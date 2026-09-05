//! End-to-end integration tests for the MUX function with float types.

e2e_f32_near!(
    end_to_end_when_mux_real_k0_then_returns_in0,
    1e-5,
    "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := MUX(0, 10.5, 20.5, 30.5);
END_PROGRAM
",
    &[(0, 10.5)],
);

e2e_f32_near!(
    end_to_end_when_mux_real_k2_then_returns_in2,
    1e-5,
    "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := MUX(2, 10.5, 20.5, 30.5);
END_PROGRAM
",
    &[(0, 30.5)],
);

e2e_f32_near!(
    end_to_end_when_mux_real_k_out_of_range_then_clamps_to_last,
    1e-5,
    "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := MUX(5, 10.5, 20.5, 30.5);
END_PROGRAM
",
    &[(0, 30.5)],
);

e2e_f64_near!(
    end_to_end_when_mux_lreal_k0_then_returns_in0,
    1e-12,
    "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := MUX(0, 10.5, 20.5, 30.5);
END_PROGRAM
",
    &[(0, 10.5)],
);

e2e_f64_near!(
    end_to_end_when_mux_lreal_k1_then_returns_in1,
    1e-12,
    "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := MUX(1, 10.5, 20.5, 30.5);
END_PROGRAM
",
    &[(0, 20.5)],
);

e2e_f64_near!(
    end_to_end_when_mux_lreal_k_out_of_range_then_clamps_to_last,
    1e-12,
    "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := MUX(5, 10.5, 20.5, 30.5);
END_PROGRAM
",
    &[(0, 30.5)],
);
