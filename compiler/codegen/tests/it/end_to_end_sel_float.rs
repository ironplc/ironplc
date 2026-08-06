//! End-to-end integration tests for the SEL function with float types.

e2e_f32_near!(
    end_to_end_when_sel_real_false_then_returns_in0,
    1e-5,
    "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := SEL(0, 10.5, 20.5);
END_PROGRAM
",
    &[(0, 10.5)],
);

e2e_f32_near!(
    end_to_end_when_sel_real_true_then_returns_in1,
    1e-5,
    "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := SEL(1, 10.5, 20.5);
END_PROGRAM
",
    &[(0, 20.5)],
);

e2e_f32_near!(
    end_to_end_when_sel_real_with_variable_then_selects,
    1e-5,
    "
PROGRAM main
  VAR
    g : DINT;
    y : REAL;
  END_VAR
  g := 1;
  y := SEL(g, 100.0, 200.0);
END_PROGRAM
",
    &[(1, 200.0)],
);

e2e_f64_near!(
    end_to_end_when_sel_lreal_false_then_returns_in0,
    1e-12,
    "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := SEL(0, 10.5, 20.5);
END_PROGRAM
",
    &[(0, 10.5)],
);

e2e_f64_near!(
    end_to_end_when_sel_lreal_true_then_returns_in1,
    1e-12,
    "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := SEL(1, 10.5, 20.5);
END_PROGRAM
",
    &[(0, 20.5)],
);
