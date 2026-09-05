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

use crate::common::assert_run_i64_with;

#[rstest]
// LDATE#2024-01-01: 19723 days * 86400 = 1704067200 seconds since epoch.
#[case::ldate_assignment(
    "
PROGRAM main
  VAR
    d : LDATE;
  END_VAR
  d := LDATE#2024-01-01;
END_PROGRAM
",
    0,
    1_704_067_200
)]
// LTOD#12:30:00: 12h * 3600000 + 30m * 60000 = 45000000 ms since midnight.
#[case::ltod_assignment(
    "
PROGRAM main
  VAR
    t : LTOD;
  END_VAR
  t := LTOD#12:30:00;
END_PROGRAM
",
    0,
    45_000_000
)]
// LDT#2024-01-01-12:30:00: 1704067200 + 45000 = 1704112200 seconds since epoch.
#[case::ldt_assignment(
    "
PROGRAM main
  VAR
    my_dt : LDT;
  END_VAR
  my_dt := LDT#2024-01-01-12:30:00;
END_PROGRAM
",
    0,
    1_704_112_200
)]
// LDATE comparison (2024-06-15 > 2024-01-01 is TRUE).
#[case::ldate_comparison(
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
    1
)]
// LTIME_OF_DAY long-form type name: LTOD#18:00:00 = 64800000 ms.
#[case::ltod_long_form(
    "
PROGRAM main
  VAR
    t : LTIME_OF_DAY;
  END_VAR
  t := LTOD#18:00:00;
END_PROGRAM
",
    0,
    64_800_000
)]
// LDATE_AND_TIME long-form type name: 19723 days * 86400 = 1704067200 s.
#[case::ldt_long_form(
    "
PROGRAM main
  VAR
    my_dt : LDATE_AND_TIME;
  END_VAR
  my_dt := LDT#2024-01-01-00:00:00;
END_PROGRAM
",
    0,
    1_704_067_200
)]
fn end_to_end_ldate(#[case] source: &str, #[case] index: usize, #[case] expected: i64) {
    assert_run_i64_with(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(index, expected)],
    );
}
