//! End-to-end tests for LTIME (64-bit duration) support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for LTIME variables and LTIME# literals. LTIME is an IEC 61131-3
//! Edition 3 (2013) feature that stores durations as 64-bit signed
//! integers in milliseconds.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::assert_run_i64_with;

#[rstest]
// LTIME#100ms stored as 100 ms (i64).
#[case::assignment_ms(
    "
PROGRAM main
  VAR
    t : LTIME;
  END_VAR
  t := LTIME#100ms;
END_PROGRAM
",
    0,
    100
)]
// LTIME#5s stored as 5000 ms.
#[case::seconds_to_ms(
    "
PROGRAM main
  VAR
    t : LTIME;
  END_VAR
  t := LTIME#5s;
END_PROGRAM
",
    0,
    5000
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
    2,
    300
)]
// Comparison of two LTIME values (5s > 3s is TRUE).
#[case::comparison(
    "
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
",
    2,
    1
)]
fn end_to_end_ltime(#[case] source: &str, #[case] index: usize, #[case] expected: i64) {
    assert_run_i64_with(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(index, expected)],
    );
}
