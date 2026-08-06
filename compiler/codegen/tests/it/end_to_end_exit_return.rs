//! End-to-end integration tests for EXIT and RETURN statement compilation.

e2e_i32!(
    end_to_end_when_exit_in_while_then_breaks_loop,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  WHILE TRUE DO
    x := x + 1;
    IF x >= 3 THEN
      EXIT;
    END_IF;
  END_WHILE;
END_PROGRAM
",
    &[(0, 3)],
);

// sum = 1 + 2 + 3 = 6 (exits when i=4, before adding)
e2e_i32!(
    end_to_end_when_exit_in_for_then_breaks_loop,
    "
PROGRAM main
  VAR
    i : DINT;
    sum : DINT;
  END_VAR
  FOR i := 1 TO 100 DO
    IF i > 3 THEN
      EXIT;
    END_IF;
    sum := sum + i;
  END_FOR;
END_PROGRAM
",
    &[(1, 6)],
);

e2e_i32!(
    end_to_end_when_exit_in_repeat_then_breaks_loop,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  REPEAT
    x := x + 1;
    IF x >= 2 THEN
      EXIT;
    END_IF;
  UNTIL FALSE
  END_REPEAT;
END_PROGRAM
",
    &[(0, 2)],
);

// Inner loop runs j=1,2 then exits at j=3, for each of i=1,2,3
// count = 3 * 2 = 6
e2e_i32!(
    end_to_end_when_exit_in_nested_loops_then_breaks_inner,
    "
PROGRAM main
  VAR
    i : DINT;
    j : DINT;
    count : DINT;
  END_VAR
  FOR i := 1 TO 3 DO
    FOR j := 1 TO 100 DO
      IF j > 2 THEN
        EXIT;
      END_IF;
      count := count + 1;
    END_FOR;
  END_FOR;
END_PROGRAM
",
    &[(2, 6)],
);

// vars[1] (y) is not assigned because RETURN skips it.
e2e_i32!(
    end_to_end_when_return_then_skips_remaining,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 42;
  RETURN;
  y := 99;
END_PROGRAM
",
    &[(0, 42), (1, 0)],
);

// vars[1] (y) is not assigned because the early RETURN skips it.
e2e_i32!(
    end_to_end_when_return_in_if_then_exits_early,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 1;
  IF x = 1 THEN
    RETURN;
  END_IF;
  y := 99;
END_PROGRAM
",
    &[(0, 1), (1, 0)],
);

// Regression: an early RETURN inside a value-returning FUNCTION used to
// emit RET_VOID, leaving the caller's stack empty and triggering a stack
// underflow when assigning the call result to a variable.
// vars[0] safe_result: early-return path; vars[1] normal_result: 10 / 3.
e2e_i32!(
    end_to_end_when_early_return_in_function_then_caller_gets_assigned_value,
    "
FUNCTION Divide : DINT
    VAR_INPUT
        numerator : DINT;
        denominator : DINT;
    END_VAR

    IF denominator = 0 THEN
        Divide := 0;
        RETURN;
    END_IF;

    Divide := numerator / denominator;
END_FUNCTION

PROGRAM main
    VAR
        safe_result : DINT;
        normal_result : DINT;
    END_VAR

    safe_result := Divide(10, 0);
    normal_result := Divide(10, 3);
END_PROGRAM
",
    &[(0, 0), (1, 3)],
);
