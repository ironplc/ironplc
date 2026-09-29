//! End-to-end integration tests for the `CONTINUE` statement (IEC 61131-3
//! Edition 3).
//!
//! `CONTINUE` jumps to where each loop goes on with its next iteration: the
//! increment of a `FOR`, the condition of a `WHILE`, the `UNTIL` of a
//! `REPEAT`. The loops have a fused (`CMP_BR`) and an unfused shape, and
//! each shape is covered.

use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_problems::Problem;

use crate::common::try_parse_and_compile;

fn edition_3() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
}

// Constant bounds: the fused head. sum = 1 + 3 + 5 + 7 + 9 = 25.
e2e_i32_with!(
    end_to_end_when_continue_in_for_then_skips_rest_of_iteration,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    sum : DINT;
  END_VAR
  FOR i := 1 TO 10 DO
    IF i MOD 2 = 0 THEN
      CONTINUE;
    END_IF;
    sum := sum + i;
  END_FOR;
END_PROGRAM
",
    &[(0, 11), (1, 25)],
);

// sum = 10 + 8 + 7 + 5 + 4 + 2 + 1 = 37 (multiples of 3 skipped).
e2e_i32_with!(
    end_to_end_when_continue_in_for_negative_step_then_skips_rest_of_iteration,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    sum : DINT;
  END_VAR
  FOR i := 10 TO 1 BY -1 DO
    IF i MOD 3 = 0 THEN
      CONTINUE;
    END_IF;
    sum := sum + i;
  END_FOR;
END_PROGRAM
",
    &[(0, 0), (1, 37)],
);

// A narrow control variable and a variable bound: the unfused head.
// sum = 1 + 2 + 4 + 5 = 12.
e2e_i32_with!(
    end_to_end_when_continue_in_for_variable_bound_then_skips_rest_of_iteration,
    edition_3(),
    "
PROGRAM main
  VAR
    i : INT;
    n : INT := 5;
    sum : INT;
  END_VAR
  FOR i := 1 TO n DO
    IF i = 3 THEN
      CONTINUE;
    END_IF;
    sum := sum + i;
  END_FOR;
END_PROGRAM
",
    &[(0, 6), (2, 12)],
);

// `i < 10` is fused. odd counts 1, 3, 5, 7, 9.
e2e_i32_with!(
    end_to_end_when_continue_in_while_then_tests_condition,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    odd : DINT;
  END_VAR
  WHILE i < 10 DO
    i := i + 1;
    IF i MOD 2 = 0 THEN
      CONTINUE;
    END_IF;
    odd := odd + 1;
  END_WHILE;
END_PROGRAM
",
    &[(0, 10), (1, 5)],
);

e2e_i32_with!(
    end_to_end_when_continue_in_while_complex_condition_then_tests_condition,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    odd : DINT;
    run : BOOL := TRUE;
  END_VAR
  WHILE run AND i < 10 DO
    i := i + 1;
    IF i MOD 2 = 0 THEN
      CONTINUE;
    END_IF;
    odd := odd + 1;
  END_WHILE;
END_PROGRAM
",
    &[(0, 10), (1, 5)],
);

// The last iteration continues (i = 10 is even), so `CONTINUE` must test
// `UNTIL` rather than restart the body, or the loop would never end.
e2e_i32_with!(
    end_to_end_when_continue_in_repeat_then_tests_until,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    odd : DINT;
  END_VAR
  REPEAT
    i := i + 1;
    IF i MOD 2 = 0 THEN
      CONTINUE;
    END_IF;
    odd := odd + 1;
  UNTIL i >= 10
  END_REPEAT;
END_PROGRAM
",
    &[(0, 10), (1, 5)],
);

e2e_i32_with!(
    end_to_end_when_continue_in_repeat_complex_until_then_tests_until,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    odd : DINT;
    stop : BOOL;
  END_VAR
  REPEAT
    i := i + 1;
    IF i MOD 2 = 0 THEN
      CONTINUE;
    END_IF;
    odd := odd + 1;
  UNTIL stop OR i >= 10
  END_REPEAT;
END_PROGRAM
",
    &[(0, 10), (1, 5)],
);

// Only the inner loop continues: inner = 3 * 3, outer = 3.
e2e_i32_with!(
    end_to_end_when_continue_in_nested_loops_then_continues_inner,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    j : DINT;
    inner : DINT;
    outer : DINT;
  END_VAR
  FOR i := 1 TO 3 DO
    FOR j := 1 TO 4 DO
      IF j = 2 THEN
        CONTINUE;
      END_IF;
      inner := inner + 1;
    END_FOR;
    outer := outer + 1;
  END_FOR;
END_PROGRAM
",
    &[(2, 9), (3, 3)],
);

// sum = 1 + 3 + 5 + 7; EXIT at 9.
e2e_i32_with!(
    end_to_end_when_continue_and_exit_in_loop_then_both_apply,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    sum : DINT;
  END_VAR
  FOR i := 1 TO 100 DO
    IF i MOD 2 = 0 THEN
      CONTINUE;
    END_IF;
    IF i > 7 THEN
      EXIT;
    END_IF;
    sum := sum + i;
  END_FOR;
END_PROGRAM
",
    &[(0, 9), (1, 16)],
);

e2e_i32_with!(
    end_to_end_when_continue_in_case_branch_then_continues_loop,
    edition_3(),
    "
PROGRAM main
  VAR
    i : DINT;
    sum : DINT;
  END_VAR
  FOR i := 1 TO 4 DO
    CASE i OF
      2, 3: CONTINUE;
    END_CASE;
    sum := sum + i;
  END_FOR;
END_PROGRAM
",
    &[(1, 5)],
);

#[test]
fn compile_when_continue_outside_loop_then_p4065_error() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
  CONTINUE;
END_PROGRAM
";
    let diagnostic = try_parse_and_compile(source, &edition_3()).unwrap_err();
    assert_eq!(diagnostic.code, Problem::ContinueOutsideLoop.code());
}
