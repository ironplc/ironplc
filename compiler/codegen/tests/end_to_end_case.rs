//! End-to-end integration tests for CASE statement compilation.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_case_matches_first_arm_then_executes_body() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 1;
  CASE x OF
    1: y := 10;
    2: y := 20;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 10);
}

#[test]
fn end_to_end_when_case_matches_second_arm_then_executes_body() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 2;
  CASE x OF
    1: y := 10;
    2: y := 20;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 2);
    assert_eq!(bufs.vars[1].as_i32(), 20);
}

#[test]
fn end_to_end_when_case_no_match_and_no_else_then_skips() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 99;
  CASE x OF
    1: y := 10;
    2: y := 20;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 99);
    assert_eq!(bufs.vars[1].as_i32(), 0); // untouched
}

#[test]
fn end_to_end_when_case_no_match_with_else_then_executes_else() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 99;
  CASE x OF
    1: y := 10;
    2: y := 20;
  ELSE
    y := 99;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 99);
    assert_eq!(bufs.vars[1].as_i32(), 99);
}

#[test]
fn end_to_end_when_case_multi_selector_then_matches_any() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 3;
  CASE x OF
    1: y := 10;
    2, 3: y := 30;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 3);
    assert_eq!(bufs.vars[1].as_i32(), 30);
}

#[test]
fn end_to_end_when_case_subrange_then_matches_in_range() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 3;
  CASE x OF
    1..5: y := 50;
    10: y := 100;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 3);
    assert_eq!(bufs.vars[1].as_i32(), 50);
}
