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

/// A count outside the range its storage holds is an internal error, not a
/// diagnostic against the program: `rule_temporal_literal_range` holds every
/// literal to the range its own type gives it, and narrowing one into shorter
/// storage is rejected too, so a count arriving here out of range means
/// analysis was skipped.
///
/// These tests reach it because the codegen tests resolve types without the
/// semantic rules. The check stays because being wrong here is silent:
/// truncating emits a different value than the program wrote -- `T#30d` became
/// a *negative* 19.7 days -- which no test of the program's behaviour would
/// attribute to codegen.
///
/// The bound is the operation type's, so the same literal is in range at the
/// 64-bit width: see `compile_when_long_literal_is_wide_enough_then_compiles`.
#[rstest]
#[case::time_past_i32_max("t : TIME;", "t := T#30d;")]
#[case::time_below_i32_min("t : TIME;", "t := T#-30d;")]
#[case::date_past_u32_max("d : DATE;", "d := D#2200-01-01;")]
#[case::date_before_epoch("d : DATE;", "d := D#1969-12-31;")]
#[case::date_and_time_past_u32_max("d : DATE_AND_TIME;", "d := DT#2200-01-01-00:00:00;")]
fn compile_when_count_exceeds_its_storage_then_internal_error(
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

    // `Problem::InternalError` is deprecated in favour of the
    // `Diagnostic::internal_error_at` constructor, so the code is named here
    // the way `compile_case` names it.
    assert_eq!(diagnostic.code, "P9998");
}

/// The boundary values themselves compile: the check rejects what the storage
/// cannot hold, not what it can.
#[rstest]
#[case::time_at_i32_max("t : TIME;", "t := T#2147483647ms;")]
#[case::time_at_i32_min("t : TIME;", "t := T#-2147483648ms;")]
#[case::date_at_epoch("d : DATE;", "d := D#1970-01-01;")]
#[case::date_at_u32_max("d : DATE;", "d := D#2106-02-07;")]
#[case::time_of_day_at_end_of_day("t : TIME_OF_DAY;", "t := TOD#23:59:59;")]
fn compile_when_count_is_at_the_boundary_then_compiles(
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

    try_parse_and_compile(&source, &CompilerOptions::default())
        .expect("the boundary value its storage holds must compile");
}

/// The 64-bit width holds what the 32-bit one cannot, because the bound comes
/// from the operation type rather than from one hardcoded width.
///
/// A full compile accepts these too, now that `rule_temporal_literal_range`
/// holds a literal to the range its own type gives it rather than to one
/// hardcoded width (issue #1560).
#[rstest]
#[case::ltime_past_i32_max("t : LTIME;", "t := LTIME#30d;")]
#[case::ldate_past_u32_max("d : LDATE;", "d := LDATE#2200-01-01;")]
#[case::ldt_past_u32_max("d : LDT;", "d := LDT#2200-01-01-00:00:00;")]
fn compile_when_long_literal_is_wide_enough_then_compiles(
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

    try_parse_and_compile(
        &source,
        &CompilerOptions::from_dialect(ironplc_parser::options::Dialect::Iec61131_3Ed3),
    )
    .expect("a 64-bit type holds a count the 32-bit one cannot");
}
