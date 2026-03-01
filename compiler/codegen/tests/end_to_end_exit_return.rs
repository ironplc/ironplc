//! End-to-end integration tests for EXIT and RETURN statement compilation.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_exit_in_while_then_breaks_loop() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 3);
}

#[test]
fn end_to_end_when_exit_in_for_then_breaks_loop() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source);
    // sum = 1 + 2 + 3 = 6 (exits when i=4, before adding)
    assert_eq!(bufs.vars[1].as_i32(), 6);
}

#[test]
fn end_to_end_when_exit_in_repeat_then_breaks_loop() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 2);
}

#[test]
fn end_to_end_when_exit_in_nested_loops_then_breaks_inner() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source);
    // Inner loop runs j=1,2 then exits at j=3, for each of i=1,2,3
    // count = 3 * 2 = 6
    assert_eq!(bufs.vars[2].as_i32(), 6);
}

#[test]
fn end_to_end_when_return_then_skips_remaining() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 42;
  RETURN;
  y := 99;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 42);
    assert_eq!(bufs.vars[1].as_i32(), 0); // y not assigned
}

#[test]
fn end_to_end_when_return_in_if_then_exits_early() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0); // y not assigned
}
