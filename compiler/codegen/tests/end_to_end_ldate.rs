//! End-to-end tests for LDATE, LTOD, and LDT (64-bit date/time) support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for long date/time variables and literals. These are IEC 61131-3
//! Edition 3 (2013) features that use 64-bit storage:
//!
//! - LDATE: stored as u64 seconds since 1970-01-01 (industry standard)
//! - LTOD (LTIME_OF_DAY): stored as u64 milliseconds since midnight
//! - LDT (LDATE_AND_TIME): stored as u64 seconds since 1970-01-01 00:00:00

mod common;
use common::parse_and_run;
use ironplc_container::VarIndex;
use ironplc_parser::options::{CompilerOptions, Dialect};

#[test]
fn end_to_end_when_ldate_assignment_then_value_is_i64_seconds_since_epoch() {
    let source = "
PROGRAM main
  VAR
    d : LDATE;
  END_VAR
  d := LDATE#2024-01-01;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // 2024-01-01 is 19723 days after 1970-01-01 = 19723 * 86400 = 1704067200
    assert_eq!(bufs.vars[0].as_i64() as u64, 1_704_067_200);
}

#[test]
fn end_to_end_when_ltod_assignment_then_value_is_i64_milliseconds_since_midnight() {
    let source = "
PROGRAM main
  VAR
    t : LTOD;
  END_VAR
  t := LTOD#12:30:00;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // 12h * 3600000 + 30m * 60000 = 45000000 ms
    assert_eq!(bufs.vars[0].as_i64() as u64, 45_000_000);
}

#[test]
fn end_to_end_when_ldt_assignment_then_value_is_i64_seconds_since_epoch() {
    let source = "
PROGRAM main
  VAR
    my_dt : LDT;
  END_VAR
  my_dt := LDT#2024-01-01-12:30:00;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // 1704067200 (date) + 12*3600 + 30*60 = 1704067200 + 45000 = 1704112200
    assert_eq!(bufs.vars[0].as_i64() as u64, 1_704_112_200);
}

#[test]
fn end_to_end_when_ldate_comparison_then_correct() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    assert_eq!(bufs.vars[2].as_i64(), 1);
}

#[test]
fn end_to_end_when_ltod_with_long_form_type_name_then_correct() {
    let source = "
PROGRAM main
  VAR
    t : LTIME_OF_DAY;
  END_VAR
  t := LTOD#18:00:00;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // 18h * 3600000 = 64800000 ms
    assert_eq!(bufs.vars[0].as_i64() as u64, 64_800_000);
}

#[test]
fn end_to_end_when_ldt_with_long_form_type_name_then_correct() {
    let source = "
PROGRAM main
  VAR
    my_dt : LDATE_AND_TIME;
  END_VAR
  my_dt := LDT#2024-01-01-00:00:00;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // 19723 days * 86400 = 1704067200 seconds
    assert_eq!(bufs.vars[0].as_i64() as u64, 1_704_067_200);
}
