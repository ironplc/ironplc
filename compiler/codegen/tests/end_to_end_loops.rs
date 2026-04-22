//! End-to-end integration tests for WHILE, REPEAT, and FOR loop statements.

#[macro_use]
mod common;

e2e_i32!(
    end_to_end_when_while_counts_down_then_correct_result,
    "PROGRAM main VAR x : DINT; END_VAR x := 5; WHILE x > 0 DO x := x - 1; END_WHILE; END_PROGRAM",
    &[(0, 0)],
);

// y (vars[1]) stays at its default 0 because the WHILE body never runs.
e2e_i32!(
    end_to_end_when_while_false_then_body_not_executed,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 0; WHILE x > 0 DO y := 99; END_WHILE; END_PROGRAM",
    &[(0, 0), (1, 0)],
);

e2e_i32!(
    end_to_end_when_repeat_counts_up_then_correct_result,
    "PROGRAM main VAR x : DINT; END_VAR REPEAT x := x + 1; UNTIL x >= 5 END_REPEAT; END_PROGRAM",
    &[(0, 5)],
);

// REPEAT checks AFTER the body, so count = 1 even with an immediately-true condition.
e2e_i32!(
    end_to_end_when_repeat_then_executes_at_least_once,
    "PROGRAM main VAR x : DINT; count : DINT; END_VAR REPEAT count := count + 1; UNTIL count >= 1 END_REPEAT; END_PROGRAM",
    &[(1, 1)],
);

// Sum 1..5 = 15.
e2e_i32!(
    end_to_end_when_for_1_to_5_then_sums_correctly,
    "PROGRAM main VAR i : DINT; sum : DINT; END_VAR FOR i := 1 TO 5 DO sum := sum + i; END_FOR; END_PROGRAM",
    &[(1, 15)],
);

// Descending: sum 5..1 by -1 = 15.
e2e_i32!(
    end_to_end_when_for_5_to_1_by_neg1_then_sums_correctly,
    "PROGRAM main VAR i : DINT; sum : DINT; END_VAR FOR i := 5 TO 1 BY -1 DO sum := sum + i; END_FOR; END_PROGRAM",
    &[(1, 15)],
);

// Step 2: i = 0,2,4,6,8,10 → 6 iterations.
e2e_i32!(
    end_to_end_when_for_with_step_2_then_iterates_correctly,
    "PROGRAM main VAR i : DINT; count : DINT; END_VAR FOR i := 0 TO 10 BY 2 DO count := count + 1; END_FOR; END_PROGRAM",
    &[(1, 6)],
);

// Empty range (10 TO 1 with implicit +1 step): body never runs, y remains 0.
e2e_i32!(
    end_to_end_when_for_empty_range_then_body_not_executed,
    "PROGRAM main VAR i : DINT; y : DINT; END_VAR FOR i := 10 TO 1 DO y := 99; END_FOR; END_PROGRAM",
    &[(1, 0)],
);
