//! End-to-end tests for DATE, TIME_OF_DAY, and DATE_AND_TIME support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for datetime variables and literals. These types are IEC 61131-3
//! Edition 2 features.
//!
//! - DATE: stored as u32 seconds since 1970-01-01 (industry standard)
//! - TIME_OF_DAY (TOD): stored as u32 milliseconds since midnight
//! - DATE_AND_TIME (DT): stored as u32 seconds since 1970-01-01

use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;

use crate::common::{parse_and_run, try_parse_and_compile};

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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 1704067200 (date) + 12*3600 + 30*60 = 1704067200 + 45000 = 1704112200
    assert_eq!(bufs.vars[0].as_i32() as u32, 1_704_112_200);
}

e2e_i32!(
    end_to_end_when_date_comparison_then_correct,
    "
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
",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_tod_comparison_then_correct,
    "
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
",
    &[(2, 1)],
);

/// A date past 2106-02-07 has no unsigned 32-bit second count, so codegen
/// reports it rather than emitting the truncated date that count would give.
///
/// `rule_date_literal_range` reports the same problem first in a full
/// compile; this path is reachable because the codegen tests resolve types
/// without running the semantic rules.
#[rstest]
#[case::date("d : DATE;", "d := D#2200-01-01;")]
#[case::date_before_epoch("d : DATE;", "d := D#1969-12-31;")]
#[case::date_and_time("d : DATE_AND_TIME;", "d := DT#2200-01-01-00:00:00;")]
fn compile_when_date_literal_is_unrepresentable_then_reports_out_of_range(
    #[case] declaration: &str,
    #[case] statement: &str,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    {declaration}
  END_VAR
  {statement}
END_PROGRAM
"
    );

    let diagnostic = try_parse_and_compile(&source, &CompilerOptions::default()).unwrap_err();

    assert_eq!(diagnostic.code, Problem::DateLiteralOutOfRange.code());
}

/// A duration, a time of day and a date are all counts rather than
/// measurements, so none has a floating-point representation, and the
/// analyzer rejects the assignment that would ask for one (P4035). Reaching
/// codegen at a float operation width is a broken invariant.
///
/// Before the match over operation widths was made exhaustive, every one of
/// these fell through to the integer arm and left the count's bit pattern in
/// a float slot.
#[rstest]
#[case::date_into_real("r : REAL;", "r := D#2024-01-01;")]
#[case::date_and_time_into_lreal("r : LREAL;", "r := DT#2024-01-01-12:30:00;")]
#[case::duration_into_real("r : REAL;", "r := T#1s;")]
#[case::duration_into_lreal("r : LREAL;", "r := T#1s;")]
#[case::time_of_day_into_real("r : REAL;", "r := TOD#12:30:00;")]
#[case::time_of_day_into_lreal("r : LREAL;", "r := TOD#12:30:00;")]
fn compile_when_time_literal_is_float_width_then_internal_error(
    #[case] declaration: &str,
    #[case] statement: &str,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    {declaration}
  END_VAR
  {statement}
END_PROGRAM
"
    );

    let diagnostic = try_parse_and_compile(&source, &CompilerOptions::default()).unwrap_err();

    // The `Problem::InternalError` variant is deprecated in favour of the
    // `Diagnostic::internal_error_at` constructor, so the code is named here
    // the way `compile_case` names it.
    assert_eq!(diagnostic.code, "P9998");
}
