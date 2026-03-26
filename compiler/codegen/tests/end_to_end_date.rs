//! End-to-end tests for DATE, TIME_OF_DAY, and DATE_AND_TIME support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for datetime variables and literals. These types are IEC 61131-3
//! Edition 2 features.
//!
//! - DATE: stored as u32 seconds since 1970-01-01 (industry standard)
//! - TIME_OF_DAY (TOD): stored as u32 milliseconds since midnight
//! - DATE_AND_TIME (DT): stored as u32 seconds since 1970-01-01

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_date_assignment_then_value_is_seconds_since_epoch() {
    let source = "
PROGRAM main
  VAR
    d : DATE;
  END_VAR
  d := D#2024-01-01;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    // 2024-01-01 is 19723 days after 1970-01-01 = 19723 * 86400 = 1704067200
    assert_eq!(bufs.vars[0].as_i32() as u32, 1_704_067_200);
}

#[test]
fn end_to_end_when_tod_assignment_then_value_is_milliseconds_since_midnight() {
    let source = "
PROGRAM main
  VAR
    t : TIME_OF_DAY;
  END_VAR
  t := TOD#12:30:00;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    // 12h * 3600000 + 30m * 60000 = 45000000 ms
    assert_eq!(bufs.vars[0].as_i32() as u32, 45_000_000);
}

#[test]
fn end_to_end_when_dt_assignment_then_value_is_seconds_since_epoch() {
    let source = "
PROGRAM main
  VAR
    my_dt : DATE_AND_TIME;
  END_VAR
  my_dt := DT#2024-01-01-12:30:00;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    // 1704067200 (date) + 12*3600 + 30*60 = 1704067200 + 45000 = 1704112200
    assert_eq!(bufs.vars[0].as_i32() as u32, 1_704_112_200);
}

#[test]
fn end_to_end_when_date_comparison_then_correct() {
    let source = "
PROGRAM main
  VAR
    a : DATE;
    b : DATE;
    result : DINT;
  END_VAR
  a := D#2024-06-15;
  b := D#2024-01-01;
  IF a > b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_tod_comparison_then_correct() {
    let source = "
PROGRAM main
  VAR
    a : TIME_OF_DAY;
    b : TIME_OF_DAY;
    result : DINT;
  END_VAR
  a := TOD#18:00:00;
  b := TOD#09:00:00;
  IF a > b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[2].as_i32(), 1);
}
