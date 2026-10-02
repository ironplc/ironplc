//! End-to-end tests for compound duration literals (`T#1m30s`).
//!
//! The parser sums the parts into one duration, so these check that the
//! stored value is that sum in milliseconds.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_run;

#[rstest]
#[case::minutes_seconds("T#1m30s", 90_000)]
#[case::fixed_point_last("T#1m1.5s", 61_500)]
#[case::underscore_in_later_number("T#1m1_000ms", 61_000)]
fn end_to_end_when_compound_duration_then_stores_sum_of_parts(
    #[case] literal: &str,
    #[case] expected_ms: i64,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    t : TIME;
  END_VAR
  t := {literal};
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i64(), expected_ms);
}
