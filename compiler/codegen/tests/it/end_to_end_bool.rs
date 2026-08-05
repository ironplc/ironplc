//! End-to-end integration tests for boolean operators.

e2e_i32!(
    end_to_end_when_bool_initial_value_true_then_variable_initialized,
    "
PROGRAM main
  VAR
    x : BOOL := TRUE;
  END_VAR
END_PROGRAM
",
    &[(0, 1)],
);

e2e_i32!(
    end_to_end_when_bool_initial_value_false_then_variable_initialized,
    "
PROGRAM main
  VAR
    x : BOOL := FALSE;
  END_VAR
END_PROGRAM
",
    &[(0, 0)],
);

e2e_i32!(
    end_to_end_when_and_both_true_then_one,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := x > 0 AND x < 10;
END_PROGRAM
",
    &[(0, 5), (1, 1)],
);

e2e_i32!(
    end_to_end_when_and_one_false_then_zero,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 15;
  y := x > 0 AND x < 10;
END_PROGRAM
",
    &[(0, 15), (1, 0)],
);

e2e_i32!(
    end_to_end_when_or_first_true_then_one,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := x > 10 OR x < 10;
END_PROGRAM
",
    &[(0, 5), (1, 1)],
);

e2e_i32!(
    end_to_end_when_or_both_false_then_zero,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := x > 10 OR x < 0;
END_PROGRAM
",
    &[(0, 5), (1, 0)],
);

e2e_i32!(
    end_to_end_when_xor_one_true_then_one,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := x > 10 XOR x < 10;
END_PROGRAM
",
    &[(0, 5), (1, 1)],
);

e2e_i32!(
    end_to_end_when_xor_both_true_then_zero,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := x > 0 XOR x < 10;
END_PROGRAM
",
    &[(0, 5), (1, 0)],
);

e2e_i32!(
    end_to_end_when_not_zero_then_one,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 0;
  y := NOT x;
END_PROGRAM
",
    &[(0, 0), (1, 1)],
);

e2e_i32!(
    end_to_end_when_not_nonzero_then_zero,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := NOT x;
END_PROGRAM
",
    &[(0, 5), (1, 0)],
);

e2e_i32!(
    end_to_end_when_true_literal_then_one,
    "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := TRUE;
END_PROGRAM
",
    &[(0, 1)],
);

e2e_i32!(
    end_to_end_when_false_literal_then_zero,
    "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := FALSE;
END_PROGRAM
",
    &[(0, 0)],
);
