//! End-to-end tests for LTIME (64-bit duration) support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for LTIME variables and LTIME# literals. LTIME is an IEC 61131-3
//! Edition 3 (2013) feature that stores durations as 64-bit signed
//! integers in milliseconds.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::{assert_run_with, Duration, FromValue};

#[rstest]
#[case::assignment_ms(
    "
PROGRAM main
  VAR
    t : LTIME;
  END_VAR
  t := LTIME#100ms;
END_PROGRAM
",
    "t",
    Duration::milliseconds(100)
)]
#[case::seconds_to_ms(
    "
PROGRAM main
  VAR
    t : LTIME;
  END_VAR
  t := LTIME#5s;
END_PROGRAM
",
    "t",
    Duration::seconds(5)
)]
// Addition of two LTIME values (100ms + 200ms = 300ms).
#[case::addition(
    "
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
",
    "c",
    Duration::milliseconds(300)
)]
fn end_to_end_ltime(#[case] source: &str, #[case] name: &str, #[case] expected: Duration) {
    assert_ed3(source, name, expected);
}

// Comparison of two LTIME values (5s > 3s is TRUE).
#[test]
fn end_to_end_ltime_comparison() {
    assert_ed3(
        "
PROGRAM main
  VAR
    a : LTIME;
    b : LTIME;
    result : LINT;
  END_VAR
  a := LTIME#5s;
  b := LTIME#3s;
  IF a > b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
",
        "result",
        1_i64,
    );
}

fn assert_ed3<T: FromValue>(source: &str, name: &str, expected: T) {
    assert_run_with(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(name, expected)],
    );
}
