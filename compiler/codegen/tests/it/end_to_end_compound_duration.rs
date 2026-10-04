//! End-to-end tests for compound duration literals (`T#1m30s`).
//!
//! The parser sums the parts into one duration, so these check that the
//! variable holds that sum.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{Duration, Snapshot};

#[rstest]
#[case::minutes_seconds("T#1m30s", Duration::seconds(90))]
#[case::fixed_point_last("T#1m1.5s", Duration::milliseconds(61_500))]
#[case::underscore_in_later_number("T#1m1_000ms", Duration::seconds(61))]
fn end_to_end_when_compound_duration_then_stores_sum_of_parts(
    #[case] literal: &str,
    #[case] expected: Duration,
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
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());

    assert_eq!(snapshot.read("t"), expected);
}
