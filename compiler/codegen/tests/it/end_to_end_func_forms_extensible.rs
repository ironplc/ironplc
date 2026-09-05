//! End-to-end integration tests for the extensible function forms of
//! operators: ADD, MUL, AND, OR and XOR called with more than two inputs.
//!
//! The two-input results are in end_to_end_func_forms.rs, and the bytecode
//! equivalence with the folded operator is pinned by the REQ-KF-codegen-002
//! conformance test; these tests run the n-input calls.

// The example from #1618.
e2e_i32!(
    end_to_end_when_and_function_with_three_bools_then_conjunction,
    "
PROGRAM main
  VAR
    b1 : BOOL;
    b2 : BOOL;
    b3 : BOOL;
    b4 : BOOL;
    b5 : BOOL;
  END_VAR
  b1 := TRUE;
  b2 := TRUE;
  b3 := FALSE;
  b4 := AND(b1, b2, b3);
  b5 := AND(b1, b2, NOT b3);
END_PROGRAM
",
    &[(3, 0), (4, 1)],
);

e2e_i32!(
    end_to_end_when_add_function_with_four_inputs_then_sum,
    "
PROGRAM main
  VAR
    x : DINT;
    result : DINT;
  END_VAR
  x := 10;
  result := ADD(x, 20, 30, 40);
END_PROGRAM
",
    &[(0, 10), (1, 100)],
);

e2e_i32!(
    end_to_end_when_mul_function_with_three_inputs_then_product,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := MUL(2, 3, 7);
END_PROGRAM
",
    &[(0, 42)],
);

e2e_i32!(
    end_to_end_when_or_function_with_three_bools_then_disjunction,
    "
PROGRAM main
  VAR
    any_true : BOOL;
    all_false : BOOL;
  END_VAR
  any_true := OR(FALSE, FALSE, TRUE);
  all_false := OR(FALSE, FALSE, FALSE);
END_PROGRAM
",
    &[(0, 1), (1, 0)],
);

// XOR over three inputs is odd parity.
e2e_i32!(
    end_to_end_when_xor_function_with_three_bools_then_parity,
    "
PROGRAM main
  VAR
    odd : BOOL;
    even : BOOL;
  END_VAR
  odd := XOR(TRUE, TRUE, TRUE);
  even := XOR(TRUE, TRUE, FALSE);
END_PROGRAM
",
    &[(0, 1), (1, 0)],
);

e2e_i32!(
    end_to_end_when_and_function_with_three_words_then_bitwise,
    "
PROGRAM main
  VAR
    result : WORD;
  END_VAR
  result := AND(WORD#16#FFFF, WORD#16#0FF0, WORD#16#00FF);
END_PROGRAM
",
    &[(0, 0x00F0)],
);

e2e_i32!(
    end_to_end_when_extensible_function_with_named_inputs_then_binds_by_number,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := ADD(IN3 := 300, IN1 := 1, IN2 := 20);
END_PROGRAM
",
    &[(0, 321)],
);
