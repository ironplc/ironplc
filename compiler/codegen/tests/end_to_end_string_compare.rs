//! End-to-end integration tests for string comparison operators.

#[macro_use]
mod common;

e2e_i32!(
    end_to_end_when_string_eq_equal_then_true,
    "PROGRAM main VAR s1 : STRING := 'hello'; s2 : STRING := 'hello'; r : BOOL; END_VAR r := s1 = s2; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_eq_different_then_false,
    "PROGRAM main VAR s1 : STRING := 'hello'; s2 : STRING := 'world'; r : BOOL; END_VAR r := s1 = s2; END_PROGRAM",
    &[(2, 0)],
);

e2e_i32!(
    end_to_end_when_string_ne_different_then_true,
    "PROGRAM main VAR s1 : STRING := 'hello'; s2 : STRING := 'world'; r : BOOL; END_VAR r := s1 <> s2; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_ne_equal_then_false,
    "PROGRAM main VAR s1 : STRING := 'abc'; s2 : STRING := 'abc'; r : BOOL; END_VAR r := s1 <> s2; END_PROGRAM",
    &[(2, 0)],
);

e2e_i32!(
    end_to_end_when_string_lt_then_correct,
    "PROGRAM main VAR s1 : STRING := 'abc'; s2 : STRING := 'abd'; r : BOOL; END_VAR r := s1 < s2; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_gt_then_correct,
    "PROGRAM main VAR s1 : STRING := 'abd'; s2 : STRING := 'abc'; r : BOOL; END_VAR r := s1 > s2; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_le_equal_then_true,
    "PROGRAM main VAR s1 : STRING := 'abc'; s2 : STRING := 'abc'; r : BOOL; END_VAR r := s1 <= s2; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_ge_equal_then_true,
    "PROGRAM main VAR s1 : STRING := 'abc'; s2 : STRING := 'abc'; r : BOOL; END_VAR r := s1 >= s2; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_shorter_prefix_then_less_than,
    "PROGRAM main VAR s1 : STRING := 'ab'; s2 : STRING := 'abc'; r : BOOL; END_VAR r := s1 < s2; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_eq_with_literal_then_correct,
    "PROGRAM main VAR s1 : STRING := 'hello'; r : BOOL; END_VAR r := s1 = 'hello'; END_PROGRAM",
    &[(1, 1)],
);

// MID('hello', 1, 3) extracts 1 char at position 3 = 'l'.
e2e_i32!(
    end_to_end_when_string_compare_mid_result_then_correct,
    "PROGRAM main VAR s : STRING := 'hello'; c : STRING[1]; r : BOOL; END_VAR c := MID(s, 1, 3); r := c = 'l'; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_empty_eq_empty_then_true,
    "PROGRAM main VAR s1 : STRING; s2 : STRING; r : BOOL; END_VAR r := s1 = s2; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_eq_in_if_then_branch_taken,
    "PROGRAM main VAR s1 : STRING := 'hello'; s2 : STRING := 'hello'; r : BOOL; END_VAR IF s1 = s2 THEN r := TRUE; ELSE r := FALSE; END_IF; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_string_ne_in_if_else_then_else_taken,
    "PROGRAM main VAR s1 : STRING := 'hello'; s2 : STRING := 'hello'; r : BOOL; END_VAR IF s1 <> s2 THEN r := TRUE; ELSE r := FALSE; END_IF; END_PROGRAM",
    &[(2, 0)],
);

e2e_i32!(
    end_to_end_when_string_eq_in_while_then_loops,
    "PROGRAM main VAR s1 : STRING := 'a'; s2 : STRING := 'a'; n : INT := 0; END_VAR WHILE s1 = s2 DO n := n + 1; IF n >= 3 THEN s2 := 'b'; END_IF; END_WHILE; END_PROGRAM",
    &[(2, 3)],
);

e2e_i32!(
    end_to_end_when_not_string_eq_in_if_then_branch_taken,
    "PROGRAM main VAR s1 : STRING := 'hello'; s2 : STRING := 'world'; r : BOOL; END_VAR IF NOT (s1 = s2) THEN r := TRUE; ELSE r := FALSE; END_IF; END_PROGRAM",
    &[(2, 1)],
);
