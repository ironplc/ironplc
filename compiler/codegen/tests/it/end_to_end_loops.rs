//! End-to-end integration tests for WHILE, REPEAT, and FOR loop statements.

e2e_i32!(
    end_to_end_when_while_counts_down_then_correct_result,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 5;
  WHILE x > 0 DO
    x := x - 1;
  END_WHILE;
END_PROGRAM
",
    &[(0, 0)],
);

// y untouched
e2e_i32!(
    end_to_end_when_while_false_then_body_not_executed,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 0;
  WHILE x > 0 DO
    y := 99;
  END_WHILE;
END_PROGRAM
",
    &[(0, 0), (1, 0)],
);

e2e_i32!(
    end_to_end_when_repeat_counts_up_then_correct_result,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  REPEAT
    x := x + 1;
  UNTIL x >= 5
  END_REPEAT;
END_PROGRAM
",
    &[(0, 5)],
);

// Even though the condition is immediately true (0 >= 0), the body executes
// once because REPEAT checks AFTER the body. count = 1 (body ran once).
e2e_i32!(
    end_to_end_when_repeat_then_executes_at_least_once,
    "
PROGRAM main
  VAR
    x : DINT;
    count : DINT;
  END_VAR
  REPEAT
    count := count + 1;
  UNTIL count >= 1
  END_REPEAT;
END_PROGRAM
",
    &[(1, 1)],
);

// 1+2+3+4+5
e2e_i32!(
    end_to_end_when_for_1_to_5_then_sums_correctly,
    "
PROGRAM main
  VAR
    i : DINT;
    sum : DINT;
  END_VAR
  FOR i := 1 TO 5 DO
    sum := sum + i;
  END_FOR;
END_PROGRAM
",
    &[(1, 15)],
);

// 5+4+3+2+1
e2e_i32!(
    end_to_end_when_for_5_to_1_by_neg1_then_sums_correctly,
    "
PROGRAM main
  VAR
    i : DINT;
    sum : DINT;
  END_VAR
  FOR i := 5 TO 1 BY -1 DO
    sum := sum + i;
  END_FOR;
END_PROGRAM
",
    &[(1, 15)],
);

// i=0,2,4,6,8,10 → 6 iterations
e2e_i32!(
    end_to_end_when_for_with_step_2_then_iterates_correctly,
    "
PROGRAM main
  VAR
    i : DINT;
    count : DINT;
  END_VAR
  FOR i := 0 TO 10 BY 2 DO
    count := count + 1;
  END_FOR;
END_PROGRAM
",
    &[(1, 6)],
);

// FOR i := 10 TO 1 DO (positive step, from > to → no iterations). y untouched.
e2e_i32!(
    end_to_end_when_for_empty_range_then_body_not_executed,
    "
PROGRAM main
  VAR
    i : DINT;
    y : DINT;
  END_VAR
  FOR i := 10 TO 1 DO
    y := 99;
  END_FOR;
END_PROGRAM
",
    &[(1, 0)],
);

// FOR-loop TRUNC elision (specs/plans/2026-04-30-elide-for-loop-trunc.md):
// the optimisation must preserve runtime behaviour for narrow integer types,
// including at the type-range boundaries where TRUNC must remain.

e2e_i32!(
    end_to_end_when_for_int_sums_then_correct_result,
    "
PROGRAM main
  VAR
    i : INT;
    sum : DINT;
  END_VAR
  FOR i := 1 TO 100 DO
    sum := sum + 1;
  END_FOR;
END_PROGRAM
",
    &[(1, 100)],
);

e2e_i32!(
    end_to_end_when_for_sint_iterates_then_correct_count,
    "
PROGRAM main
  VAR
    i : SINT;
    count : DINT;
  END_VAR
  FOR i := 1 TO 10 DO
    count := count + 1;
  END_FOR;
END_PROGRAM
",
    &[(1, 10)],
);

e2e_i32!(
    end_to_end_when_for_uint_iterates_then_correct_count,
    "
PROGRAM main
  VAR
    i : UINT;
    count : DINT;
  END_VAR
  FOR i := 1 TO 50 DO
    count := count + 1;
  END_FOR;
END_PROGRAM
",
    &[(1, 50)],
);

e2e_i32!(
    end_to_end_when_for_int_negative_step_then_correct_count,
    "
PROGRAM main
  VAR
    i : INT;
    count : DINT;
  END_VAR
  FOR i := 100 TO 1 BY -1 DO
    count := count + 1;
  END_FOR;
END_PROGRAM
",
    &[(1, 100)],
);
