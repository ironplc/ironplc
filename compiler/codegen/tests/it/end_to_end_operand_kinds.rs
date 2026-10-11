//! End-to-end tests for the operators whose operands are not two numbers of
//! one type: a duration scaled by or divided by a number, two times or two
//! dates and times subtracted, a real raised to an integer power, and two
//! wide strings ordered.
//!
//! The operators between numbers of one type, at every type, are in
//! `end_to_end_operator_matrix.rs`.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{time, Duration, Snapshot, Time, Value};

/// Assigns `expr` to `result` of `result_type` and returns what it holds.
fn evaluate(result_type: &str, expr: &str) -> Value {
    let source = format!(
        "
PROGRAM main
  VAR
    result : {result_type};
    five : TIME := T#5s; two : TIME := T#2s; count : INT := 3;
    morning : TOD := TOD#10:00:00; midday : TOD := TOD#12:30:00;
    noon : DT := DT#2024-01-01-12:00:00; one_pm : DT := DT#2024-01-01-13:00:00;
    base : REAL := 2.0;
    first : WSTRING[5] := \"abc\"; second : WSTRING[5] := \"abd\";
  END_VAR
  result := {expr};
END_PROGRAM
"
    );
    Snapshot::run(&source, &CompilerOptions::default()).read("result")
}

#[rstest]
#[case::negated("-five", Duration::seconds(-5))]
#[case::difference_below_zero("two - five", Duration::seconds(-3))]
#[case::subtracted_by_function("SUB(five, two)", Duration::seconds(3))]
#[case::times_integer_variable("five * count", Duration::seconds(15))]
// A duration divided by an integer keeps its milliseconds.
#[case::divided_by_integer("five / 2", Duration::milliseconds(2500))]
#[case::time_of_day_minus_time_of_day("midday - morning", Duration::minutes(150))]
#[case::date_and_time_minus_date_and_time("one_pm - noon", Duration::hours(1))]
fn end_to_end_when_durations_and_times_combined_then_result_is_a_duration(
    #[case] expr: &str,
    #[case] expected: Duration,
) {
    assert_eq!(evaluate("TIME", expr), expected);
}

#[rstest]
#[case::time_of_day_plus_duration("morning + five", time!(10:00:05))]
#[case::time_of_day_minus_duration("morning - five", time!(9:59:55))]
fn end_to_end_when_duration_added_to_time_of_day_then_result_is_a_time_of_day(
    #[case] expr: &str,
    #[case] expected: Time,
) {
    assert_eq!(evaluate("TOD", expr), expected);
}

#[rstest]
#[case::integer_variable_power("REAL", "base ** count", 8.0)]
#[case::negative_power("REAL", "base ** (-1)", 0.5)]
#[case::function_with_integer_power("LREAL", "EXPT(LREAL#2.0, 10)", 1024.0)]
fn end_to_end_when_real_raised_to_integer_power_then_result_is_real(
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: f64,
) {
    assert_eq!(evaluate(result_type, expr), expected);
}

#[rstest]
#[case::less("first < second", true)]
#[case::not_equal("first <> second", true)]
#[case::greater_or_equal_by_function("GE(first, second)", false)]
fn end_to_end_when_wide_strings_ordered_then_compares_code_units(
    #[case] expr: &str,
    #[case] expected: bool,
) {
    assert_eq!(evaluate("BOOL", expr), expected);
}
