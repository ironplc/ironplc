//! End-to-end integration tests for comparison operators.

e2e_i32!(
    end_to_end_when_eq_true_then_one,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 5; y := x = 5; END_PROGRAM",
    &[(0, 5), (1, 1)],
);

e2e_i32!(
    end_to_end_when_ne_true_then_one,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 5; y := x <> 3; END_PROGRAM",
    &[(0, 5), (1, 1)],
);

e2e_i32!(
    end_to_end_when_lt_true_then_one,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 3; y := x < 5; END_PROGRAM",
    &[(0, 3), (1, 1)],
);

e2e_i32!(
    end_to_end_when_le_equal_then_one,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 5; y := x <= 5; END_PROGRAM",
    &[(0, 5), (1, 1)],
);

e2e_i32!(
    end_to_end_when_gt_true_then_one,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 7; y := x > 5; END_PROGRAM",
    &[(0, 7), (1, 1)],
);

e2e_i32!(
    end_to_end_when_ge_false_then_zero,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 3; y := x >= 5; END_PROGRAM",
    &[(0, 3), (1, 0)],
);

// -2.5 < 0.0 is TRUE (1); 3.5 < 0.0 is FALSE (0)
e2e_i32!(
    end_to_end_when_real_lt_assigned_to_bool_then_correct,
    "PROGRAM main VAR x : REAL; neg : BOOL; pos : BOOL; END_VAR x := -2.5; neg := x < 0.0; x := 3.5; pos := x < 0.0; END_PROGRAM",
    &[(1, 1), (2, 0)],
);

e2e_i32!(
    end_to_end_when_real_gt_assigned_to_bool_then_correct,
    "PROGRAM main VAR x : REAL; result : BOOL; END_VAR x := 1.5; result := x > 0.0; END_PROGRAM",
    &[(1, 1)],
);

// 72 >= 65 is TRUE
e2e_i32!(
    end_to_end_when_byte_ge_true_then_one,
    "PROGRAM main VAR c : BYTE; result : BOOL; END_VAR c := BYTE#72; result := c >= BYTE#65; END_PROGRAM",
    &[(1, 1)],
);

// 72 <= 90 is TRUE
e2e_i32!(
    end_to_end_when_byte_le_true_then_one,
    "PROGRAM main VAR c : BYTE; result : BOOL; END_VAR c := BYTE#72; result := c <= BYTE#90; END_PROGRAM",
    &[(1, 1)],
);

// 50 > 65 is FALSE
e2e_i32!(
    end_to_end_when_byte_gt_false_then_zero,
    "PROGRAM main VAR c : BYTE; result : BOOL; END_VAR c := BYTE#50; result := c > BYTE#65; END_PROGRAM",
    &[(1, 0)],
);

// 200 < 100 is FALSE (unsigned comparison)
e2e_i32!(
    end_to_end_when_byte_lt_false_then_zero,
    "PROGRAM main VAR c : BYTE; result : BOOL; END_VAR c := BYTE#200; result := c < BYTE#100; END_PROGRAM",
    &[(1, 0)],
);

// 'H' (72) is uppercase; 'a' (97) is not uppercase
e2e_i32!(
    end_to_end_when_byte_range_check_then_correct,
    "FUNCTION IS_UPPERCASE : BOOL VAR_INPUT c : BYTE; END_VAR IS_UPPERCASE := c >= BYTE#65 AND c <= BYTE#90; END_FUNCTION PROGRAM main VAR yes : BOOL; no : BOOL; END_VAR yes := IS_UPPERCASE(c := BYTE#72); no := IS_UPPERCASE(c := BYTE#97); END_PROGRAM",
    &[(0, 1), (1, 0)],
);
