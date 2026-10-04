//! End-to-end tests for DATE, TIME_OF_DAY, and DATE_AND_TIME support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for datetime variables and literals. These types are IEC 61131-3
//! Edition 2 features.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{date, datetime, time, try_parse_and_compile, Snapshot};

#[test]
fn end_to_end_when_date_assignment_then_reads_date() {
    let source = "
PROGRAM main
  VAR
    d : DATE;
  END_VAR
  d := D#2024-01-01;
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read("d"), date!(2024 - 01 - 01));
}

#[test]
fn end_to_end_when_tod_assignment_then_reads_time_of_day() {
    let source = "
PROGRAM main
  VAR
    t : TIME_OF_DAY;
  END_VAR
  t := TOD#12:30:00;
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read("t"), time!(12:30));
}

#[test]
fn end_to_end_when_dt_assignment_then_reads_date_and_time() {
    let source = "
PROGRAM main
  VAR
    my_dt : DATE_AND_TIME;
  END_VAR
  my_dt := DT#2024-01-01-12:30:00;
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read("my_dt"), datetime!(2024-01-01 12:30));
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
    &[("result", 1)],
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
    &[("result", 1)],
);

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
