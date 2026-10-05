//! End-to-end tests for a duration literal written with a fractional unit.
//!
//! `T#1.5h` is one and a half hours. Computing that means scaling the
//! literal's femtosecond fraction by the unit's length in seconds, which the
//! shared day/hour/minute constructor got wrong in two ways: the scaled value
//! is a count of seconds but was handed to `Duration::microseconds`, so
//! `T#1.5h` came out as one hour and 1.8 milliseconds; and the numerator
//! overflowed a `u64` for a fractional day, so `T#1.5d` panicked the compiler
//! in a debug build and wrapped in a release one.
//!
//! A whole-numbered literal has no fraction, so nothing here was covered until
//! these tests: every existing case was `T#1h`, never `T#1.5h`.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{assert_run_with, Duration};

#[rstest]
// 1.5 days = 36 hours.
#[case::half_day("T#1.5d", Duration::hours(36))]
// 1.5 hours = 90 minutes.
#[case::half_hour("T#1.5h", Duration::minutes(90))]
// 1.5 minutes = 90 seconds.
#[case::half_minute("T#1.5m", Duration::seconds(90))]
#[case::half_second("T#1.5s", Duration::milliseconds(1500))]
// 1.5 milliseconds truncates to the millisecond the storage counts in.
#[case::half_millisecond("T#1.5ms", Duration::milliseconds(1))]
// A whole unit is unchanged: the fraction is zero.
#[case::whole_day("T#1d", Duration::days(1))]
#[case::whole_hour("T#1h", Duration::hours(1))]
#[case::whole_minute("T#1m", Duration::minutes(1))]
// The smallest fraction each unit can carry still scales by the unit.
#[case::quarter_day("T#0.25d", Duration::hours(6))]
#[case::tenth_hour("T#0.1h", Duration::minutes(6))]
fn end_to_end_when_duration_has_a_fractional_unit_then_scales_by_the_unit(
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

    assert_run_with(&source, &CompilerOptions::default(), &[("t", expected)]);
}
