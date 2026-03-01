//! End-to-end integration tests for WHILE, REPEAT, and FOR loop statements.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_while_counts_down_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 5;
  WHILE x > 0 DO
    x := x - 1;
  END_WHILE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 0);
}

#[test]
fn end_to_end_when_while_false_then_body_not_executed() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 0;
  WHILE x > 0 DO
    y := 99;
  END_WHILE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 0);
    assert_eq!(bufs.vars[1].as_i32(), 0); // y untouched
}

#[test]
fn end_to_end_when_repeat_counts_up_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  REPEAT
    x := x + 1;
  UNTIL x >= 5
  END_REPEAT;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 5);
}

#[test]
fn end_to_end_when_repeat_then_executes_at_least_once() {
    // Even though the condition is immediately true (0 >= 0),
    // the body executes once because REPEAT checks AFTER the body.
    let source = "
PROGRAM main
  VAR
    x : INT;
    count : INT;
  END_VAR
  REPEAT
    count := count + 1;
  UNTIL count >= 1
  END_REPEAT;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 1); // count = 1 (body ran once)
}

#[test]
fn end_to_end_when_for_1_to_5_then_sums_correctly() {
    let source = "
PROGRAM main
  VAR
    i : INT;
    sum : INT;
  END_VAR
  FOR i := 1 TO 5 DO
    sum := sum + i;
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 15); // 1+2+3+4+5
}

#[test]
fn end_to_end_when_for_5_to_1_by_neg1_then_sums_correctly() {
    let source = "
PROGRAM main
  VAR
    i : INT;
    sum : INT;
  END_VAR
  FOR i := 5 TO 1 BY -1 DO
    sum := sum + i;
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 15); // 5+4+3+2+1
}

#[test]
fn end_to_end_when_for_with_step_2_then_iterates_correctly() {
    let source = "
PROGRAM main
  VAR
    i : INT;
    count : INT;
  END_VAR
  FOR i := 0 TO 10 BY 2 DO
    count := count + 1;
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 6); // i=0,2,4,6,8,10 → 6 iterations
}

#[test]
fn end_to_end_when_for_empty_range_then_body_not_executed() {
    // FOR i := 10 TO 1 DO (positive step, from > to → no iterations)
    let source = "
PROGRAM main
  VAR
    i : INT;
    y : INT;
  END_VAR
  FOR i := 10 TO 1 DO
    y := 99;
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 0); // y untouched
}
