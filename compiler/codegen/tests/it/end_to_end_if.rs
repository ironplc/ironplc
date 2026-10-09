//! End-to-end integration tests for IF/ELSIF/ELSE statements.

use spec_test_macro::spec_test;

e2e_i32!(
    end_to_end_when_if_true_then_executes_body,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  IF x > 0 THEN
    y := 1;
  END_IF;
END_PROGRAM
",
    &[("x", 5), ("y", 1)],
);

// y is untouched.
e2e_i32!(
    end_to_end_when_if_false_then_skips_body,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := -5;
  IF x > 0 THEN
    y := 1;
  END_IF;
END_PROGRAM
",
    &[("x", -5), ("y", 0)],
);

e2e_i32!(
    end_to_end_when_if_else_true_then_executes_then,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  IF x > 0 THEN
    y := 1;
  ELSE
    y := 2;
  END_IF;
END_PROGRAM
",
    &[("x", 5), ("y", 1)],
);

e2e_i32!(
    end_to_end_when_if_else_false_then_executes_else,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := -5;
  IF x > 0 THEN
    y := 1;
  ELSE
    y := 2;
  END_IF;
END_PROGRAM
",
    &[("x", -5), ("y", 2)],
);

e2e_i32!(
    end_to_end_when_if_elsif_else_first_true_then_executes_first,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  IF x > 5 THEN
    y := 1;
  ELSIF x > 0 THEN
    y := 2;
  ELSE
    y := 3;
  END_IF;
END_PROGRAM
",
    &[("x", 10), ("y", 1)],
);

e2e_i32!(
    end_to_end_when_if_elsif_else_second_true_then_executes_second,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 3;
  IF x > 5 THEN
    y := 1;
  ELSIF x > 0 THEN
    y := 2;
  ELSE
    y := 3;
  END_IF;
END_PROGRAM
",
    &[("x", 3), ("y", 2)],
);

e2e_i32!(
    end_to_end_when_if_elsif_else_none_true_then_executes_else,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := -5;
  IF x > 5 THEN
    y := 1;
  ELSIF x > 0 THEN
    y := 2;
  ELSE
    y := 3;
  END_IF;
END_PROGRAM
",
    &[("x", -5), ("y", 3)],
);

// n defaults to 0, so 2 > 0 is true.
e2e_i32!(
    end_to_end_when_if_literal_gt_var_true_then_executes_body,
    "
PROGRAM main
  VAR
    n : DINT;
    y : DINT;
  END_VAR
  IF 2 > n THEN
    y := 1;
  END_IF;
END_PROGRAM
",
    &[("y", 1)],
);

// n is 5, so 2 > 5 is false.
e2e_i32!(
    end_to_end_when_if_literal_gt_var_false_then_skips_body,
    "
PROGRAM main
  VAR
    n : DINT;
    y : DINT;
  END_VAR
  n := 5;
  IF 2 > n THEN
    y := 1;
  END_IF;
END_PROGRAM
",
    &[("n", 5), ("y", 0)],
);

// 2 * 4 = 8, and 8 > 8 is false.
e2e_i32!(
    end_to_end_when_if_literal_expr_gt_literal_false_then_skips_body,
    "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  IF 2 * 4 > 8 THEN
    y := 1;
  END_IF;
END_PROGRAM
",
    &[("y", 0)],
);

// A condition is a BOOL whatever its comparison compares, so NOT negates the
// BOOL rather than the bits of the comparison's DWORD operands: 5 > 3 is
// TRUE, and NOT of it skips the body.
e2e_i32!(
    #[spec_test(REQ_IC_codegen_007)]
    end_to_end_when_if_not_of_dword_comparison_true_then_skips_body,
    "
PROGRAM main
  VAR
    a : DWORD := 5;
    b : DWORD := 3;
    y : DINT;
  END_VAR
  IF NOT (a > b) THEN
    y := 1;
  END_IF;
END_PROGRAM
",
    &[("y", 0)],
);

e2e_i32!(
    #[spec_test(REQ_IC_codegen_007)]
    end_to_end_when_if_not_of_lword_comparison_true_then_skips_body,
    "
PROGRAM main
  VAR
    a : LWORD := 5;
    b : LWORD := 3;
    y : DINT;
  END_VAR
  IF NOT (a > b) THEN
    y := 1;
  END_IF;
END_PROGRAM
",
    &[("y", 0)],
);

e2e_i32!(
    end_to_end_when_elsif_not_of_dword_comparison_true_then_skips_branch,
    "
PROGRAM main
  VAR
    a : DWORD := 5;
    b : DWORD := 3;
    y : DINT;
  END_VAR
  IF FALSE THEN
    y := 1;
  ELSIF NOT (a > b) THEN
    y := 2;
  ELSE
    y := 3;
  END_IF;
END_PROGRAM
",
    &[("y", 3)],
);
