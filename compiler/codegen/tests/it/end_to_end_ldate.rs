//! End-to-end tests for LDATE, LTOD, and LDT (64-bit date/time) support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for long date/time variables and literals. These are IEC 61131-3
//! Edition 3 (2013) features that use 64-bit storage:
//!
//! - LDATE: stored as u64 seconds since 1970-01-01 (industry standard)
//! - LTOD (LTIME_OF_DAY): stored as u64 milliseconds since midnight
//! - LDT (LDATE_AND_TIME): stored as u64 seconds since 1970-01-01 00:00:00

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::{
    assert_run_with, date, datetime, time, Date, PrimitiveDateTime, SlotValue, Time,
};

fn assert_ed3<T: SlotValue>(source: &str, index: usize, expected: T) {
    assert_run_with(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(index, expected)],
    );
}

#[rstest]
#[case::ldate_assignment(
    "
PROGRAM main
  VAR
    d : LDATE;
  END_VAR
  d := LDATE#2024-01-01;
END_PROGRAM
",
    date!(2024-01-01)
)]
fn end_to_end_ldate(#[case] source: &str, #[case] expected: Date) {
    assert_ed3(source, 0, expected);
}

#[rstest]
#[case::ltod_assignment(
    "
PROGRAM main
  VAR
    t : LTOD;
  END_VAR
  t := LTOD#12:30:00;
END_PROGRAM
",
    time!(12:30)
)]
// LTIME_OF_DAY long-form type name.
#[case::ltod_long_form(
    "
PROGRAM main
  VAR
    t : LTIME_OF_DAY;
  END_VAR
  t := LTOD#18:00:00;
END_PROGRAM
",
    time!(18:00)
)]
fn end_to_end_ltod(#[case] source: &str, #[case] expected: Time) {
    assert_ed3(source, 0, expected);
}

#[rstest]
#[case::ldt_assignment(
    "
PROGRAM main
  VAR
    my_dt : LDT;
  END_VAR
  my_dt := LDT#2024-01-01-12:30:00;
END_PROGRAM
",
    datetime!(2024-01-01 12:30)
)]
// LDATE_AND_TIME long-form type name.
#[case::ldt_long_form(
    "
PROGRAM main
  VAR
    my_dt : LDATE_AND_TIME;
  END_VAR
  my_dt := LDT#2024-01-01-00:00:00;
END_PROGRAM
",
    datetime!(2024-01-01 0:00)
)]
fn end_to_end_ldt(#[case] source: &str, #[case] expected: PrimitiveDateTime) {
    assert_ed3(source, 0, expected);
}

// LDATE comparison (2024-06-15 > 2024-01-01 is TRUE).
#[test]
fn end_to_end_ldate_comparison() {
    assert_ed3(
        "
PROGRAM main
  VAR
    a : LDATE;
    b : LDATE;
    result : LINT;
  END_VAR
  a := LDATE#2024-06-15;
  b := LDATE#2024-01-01;
  IF a > b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
",
        2,
        1_i64,
    );
}
