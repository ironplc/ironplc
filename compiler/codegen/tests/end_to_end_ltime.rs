//! End-to-end tests for LTIME (64-bit duration) support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for LTIME variables and LTIME# literals. LTIME is an IEC 61131-3
//! Edition 3 (2013) feature that stores durations as 64-bit signed
//! integers in milliseconds.

mod common;
use common::parse_and_run;
use ironplc_parser::options::{Dialect, ParseOptions};

#[test]
fn end_to_end_when_ltime_assignment_then_value_is_i64_milliseconds() {
    let source = "
PROGRAM main
  VAR
    t : LTIME;
  END_VAR
  t := LTIME#100ms;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::from_dialect(Dialect::Iec61131_3Ed3));
    assert_eq!(bufs.vars[0].as_i64(), 100);
}

#[test]
fn end_to_end_when_ltime_seconds_then_correct_milliseconds() {
    let source = "
PROGRAM main
  VAR
    t : LTIME;
  END_VAR
  t := LTIME#5s;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::from_dialect(Dialect::Iec61131_3Ed3));
    assert_eq!(bufs.vars[0].as_i64(), 5000);
}

#[test]
fn end_to_end_when_ltime_addition_then_correct() {
    let source = "
PROGRAM main
  VAR
    a : LTIME;
    b : LTIME;
    c : LTIME;
  END_VAR
  a := LTIME#100ms;
  b := LTIME#200ms;
  c := a + b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::from_dialect(Dialect::Iec61131_3Ed3));
    assert_eq!(bufs.vars[2].as_i64(), 300);
}

#[test]
fn end_to_end_when_ltime_comparison_then_correct() {
    let source = "
PROGRAM main
  VAR
    a : LTIME;
    b : LTIME;
    result : LTIME;
  END_VAR
  a := LTIME#5s;
  b := LTIME#3s;
  IF a > b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::from_dialect(Dialect::Iec61131_3Ed3));
    assert_eq!(bufs.vars[2].as_i64(), 1);
}
