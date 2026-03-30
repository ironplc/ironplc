//! End-to-end integration tests for the FIND standard function.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_find_substring_then_returns_position() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'World';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // 'World' starts at position 7 (1-based).
    assert_eq!(bufs.vars[2].as_i32(), 7);
}

#[test]
fn end_to_end_when_find_not_found_then_returns_zero() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'XYZ';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[2].as_i32(), 0);
}

#[test]
fn end_to_end_when_find_at_start_then_returns_one() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'H';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_find_empty_search_then_returns_zero() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    s2 : STRING;
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[2].as_i32(), 0);
}

#[test]
fn end_to_end_when_find_exact_match_then_returns_one() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'abc';
    s2 : STRING := 'abc';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_find_search_longer_than_haystack_then_returns_zero() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hi';
    s2 : STRING := 'Hello';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[2].as_i32(), 0);
}

#[test]
fn end_to_end_when_find_at_end_then_returns_correct_position() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'DE';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // 'DE' starts at position 4 (1-based).
    assert_eq!(bufs.vars[2].as_i32(), 4);
}
