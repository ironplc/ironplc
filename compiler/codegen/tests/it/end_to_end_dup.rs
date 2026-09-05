//! End-to-end execution tests verifying DUP optimizations produce correct results.

e2e_i32!(
    end_to_end_when_var_squared_then_correct_result,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := x * x;
END_PROGRAM
",
    &[(0, 5), (1, 25)],
);

e2e_i32!(
    end_to_end_when_store_load_optimized_then_correct_values,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 7;
  y := x + 3;
END_PROGRAM
",
    &[(0, 7), (1, 10)],
);

e2e_i32!(
    end_to_end_when_chain_of_assignments_then_all_correct,
    "
PROGRAM main
  VAR
    a : DINT;
    b : DINT;
    c : DINT;
  END_VAR
  a := 10;
  b := a + 5;
  c := b * 2;
END_PROGRAM
",
    &[(0, 10), (1, 15), (2, 30)],
);

e2e_i32!(
    end_to_end_when_var_doubled_then_correct_result,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 7;
  y := x + x;
END_PROGRAM
",
    &[(0, 7), (1, 14)],
);
