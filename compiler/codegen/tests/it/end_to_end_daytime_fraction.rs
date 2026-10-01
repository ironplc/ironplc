//! End-to-end tests for the fractional seconds of time-of-day and
//! date-and-time literals (issue #1921).
//!
//! The fraction reaches the stored count at the type's own unit
//! (ADR-0025): milliseconds for `TIME_OF_DAY` and `LTIME_OF_DAY`, seconds
//! for `DATE_AND_TIME` and `LDATE_AND_TIME`. A finer fraction is truncated,
//! as a sub-millisecond `TIME` literal is (ADR-0021).

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::{assert_run_i64_with, parse_and_run};

fn edition3() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
}

#[rstest]
// The program from issue #1921: 10h = 36,000,000 ms, plus 250 ms.
#[case::tod_initializer(
    "
PROGRAM main
  VAR
    a : TIME_OF_DAY := TOD#10:00:00.250;
    ms : DINT;
  END_VAR
  ms := TOD_TO_DINT(a);
END_PROGRAM
",
    1,
    36_000_250
)]
#[case::time_of_day_assignment(
    "
PROGRAM main
  VAR
    a : TIME_OF_DAY;
  END_VAR
  a := TIME_OF_DAY#23:59:59.999;
END_PROGRAM
",
    0,
    86_399_999
)]
// A fraction below the millisecond is truncated, as for `T#`.
#[case::tod_sub_millisecond_truncated(
    "
PROGRAM main
  VAR
    a : TIME_OF_DAY := TOD#10:00:00.0009;
  END_VAR
  a := a;
END_PROGRAM
",
    0,
    36_000_000
)]
#[case::tod_constant(
    "
PROGRAM main
  VAR CONSTANT
    start : TIME_OF_DAY := TOD#10:00:00.5;
  END_VAR
  VAR
    a : TIME_OF_DAY;
  END_VAR
  a := start;
END_PROGRAM
",
    1,
    36_000_500
)]
#[case::tod_plus_time(
    "
PROGRAM main
  VAR
    a : TIME_OF_DAY;
  END_VAR
  a := TOD#10:00:00.250 + T#250ms;
END_PROGRAM
",
    0,
    36_000_500
)]
// Before the fix both literals were 10:00:00, so the comparison was FALSE.
#[case::tod_comparison(
    "
PROGRAM main
  VAR
    later : BOOL;
  END_VAR
  later := TOD#10:00:00.250 > TOD#10:00:00.100;
END_PROGRAM
",
    0,
    1
)]
// 2024-01-02 is 1,704,153,600 s; 10h adds 36,000 s. The 0.75 s fraction is
// below the one-second unit and is truncated.
#[case::dt_fraction_truncated(
    "
PROGRAM main
  VAR
    d : DATE_AND_TIME := DT#2024-01-02-10:00:00.75;
  END_VAR
  d := d;
END_PROGRAM
",
    0,
    1_704_189_600
)]
#[case::date_and_time_fraction_truncated(
    "
PROGRAM main
  VAR
    d : DATE_AND_TIME;
  END_VAR
  d := DATE_AND_TIME#2024-01-02-10:00:00.75;
END_PROGRAM
",
    0,
    1_704_189_600
)]
fn end_to_end_when_short_daytime_literal_has_fraction_then_stored_at_type_unit(
    #[case] source: &str,
    #[case] index: usize,
    #[case] expected: u32,
) {
    let (_c, bufs) = parse_and_run(source, &edition3());
    assert_eq!(bufs.vars[index].as_i32() as u32, expected);
}

#[rstest]
#[case::ltod_initializer(
    "
PROGRAM main
  VAR
    a : LTIME_OF_DAY := LTOD#10:00:00.250;
  END_VAR
  a := a;
END_PROGRAM
",
    36_000_250
)]
#[case::ltime_of_day_assignment(
    "
PROGRAM main
  VAR
    a : LTIME_OF_DAY;
  END_VAR
  a := LTIME_OF_DAY#23:59:59.999;
END_PROGRAM
",
    86_399_999
)]
#[case::ldt_fraction_truncated(
    "
PROGRAM main
  VAR
    d : LDT := LDT#2024-01-02-10:00:00.75;
  END_VAR
  d := d;
END_PROGRAM
",
    1_704_189_600
)]
#[case::ldate_and_time_fraction_truncated(
    "
PROGRAM main
  VAR
    d : LDATE_AND_TIME;
  END_VAR
  d := LDATE_AND_TIME#2024-01-02-10:00:00.75;
END_PROGRAM
",
    1_704_189_600
)]
fn end_to_end_when_long_daytime_literal_has_fraction_then_stored_at_type_unit(
    #[case] source: &str,
    #[case] expected: i64,
) {
    assert_run_i64_with(source, &edition3(), &[(0, expected)]);
}
