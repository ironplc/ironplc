//! End-to-end tests for a short time or date value widened to 64 bits (#1877).
//!
//! A 32-bit value lives in its slot sign-extended, so widening it is a no-op
//! for a signed type and a zero-extension for an unsigned one. `DATE`,
//! `TIME_OF_DAY` and `DATE_AND_TIME` are unsigned (ADR-0025) and `TIME` is
//! signed (ADR-0021), so a date after 2038 — whose top bit is set — must reach
//! a long target zero-extended however it is written, and a negative `TIME`
//! must reach an `LTIME` sign-extended.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::assert_run_i64_with;

/// 2100-01-01-00:00:00 in seconds since the epoch. Above 2^31, so its top
/// bit is set as a 32-bit value.
const LATE: i64 = 4_102_444_800;

/// A program assigning `expr` to `result` of `result_type`, the first
/// variable (index 0), with short temporal operands past 2038.
fn program(result_type: &str, expr: &str) -> String {
    format!(
        "
TYPE
  STAMP : STRUCT
    moment : DATE_AND_TIME;
  END_STRUCT;
END_TYPE
FUNCTION late_dt : DATE_AND_TIME
  late_dt := DT#2100-01-01-00:00:00;
END_FUNCTION
FUNCTION late_udint : UDINT
  late_udint := 4000000000;
END_FUNCTION
FUNCTION echo_ldt : LDATE_AND_TIME
  VAR_INPUT
    v : LDATE_AND_TIME;
  END_VAR
  echo_ldt := v;
END_FUNCTION
PROGRAM main
  VAR
    result : {result_type};
    dt_late : DATE_AND_TIME := DT#2100-01-01-00:00:00;
    d_late : DATE := D#2100-01-01;
    tod_ten : TIME_OF_DAY := TOD#10:00:00;
    tod_last : TIME_OF_DAY := TOD#23:59:59;
    t : TIME := T#1h;
    t_neg : TIME := T#-5s;
    stamps : ARRAY[1..2] OF DATE_AND_TIME := [DT#2100-01-01-00:00:00, DT#2100-01-01-00:00:00];
    stamp : STAMP;
    ldt_hour_later : LDATE_AND_TIME := LDT#2100-01-01-01:00:00;
  END_VAR
  stamp.moment := dt_late;
  result := {expr};
END_PROGRAM
"
    )
}

fn assert_result(result_type: &str, expr: &str, expected: i64) {
    assert_run_i64_with(
        &program(result_type, expr),
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(0, expected)],
    );
}

/// REQ-AO-codegen-013: the result of an operator expression or a typed call
/// on short operands is widened to a long target by its own signedness.
#[spec_test(REQ_AO_codegen_013)]
#[rstest]
#[case::dt_typed_add("LDATE_AND_TIME", "ADD_DT_TIME(dt_late, t)", LATE + 3600)]
#[case::dt_operator_add("LDATE_AND_TIME", "dt_late + t", LATE + 3600)]
#[case::dt_function_form_add("LDATE_AND_TIME", "ADD(dt_late, t)", LATE + 3600)]
#[case::dt_typed_sub("LDATE_AND_TIME", "SUB_DT_TIME(dt_late, t)", LATE - 3600)]
#[case::dt_operator_sub("LDATE_AND_TIME", "dt_late - t", LATE - 3600)]
#[case::dt_concat_date_tod("LDATE_AND_TIME", "CONCAT_DATE_TOD(d_late, tod_ten)", LATE + 36_000)]
#[case::tod_typed_add("LTIME_OF_DAY", "ADD_TOD_TIME(tod_ten, t)", 39_600_000)]
#[case::tod_operator_add("LTIME_OF_DAY", "tod_ten + t", 39_600_000)]
#[case::time_typed_add("LTIME", "ADD_TIME(t_neg, t_neg)", -10_000)]
#[case::time_operator_add("LTIME", "t_neg + t_neg", -10_000)]
fn end_to_end_req_ao_013_when_short_result_stored_into_long_target_then_widened_by_signedness(
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_result(result_type, expr, expected);
}

#[rstest]
#[case::variable("dt_late")]
#[case::parenthesized("(dt_late)")]
#[case::literal("DT#2100-01-01-00:00:00")]
#[case::conversion("DT_TO_LDT(dt_late)")]
#[case::move_call("MOVE(dt_late)")]
#[case::sel_call("SEL(TRUE, dt_late, dt_late)")]
#[case::array_element("stamps[1]")]
#[case::struct_field("stamp.moment")]
#[case::user_function("late_dt()")]
#[case::user_function_long_argument("echo_ldt(dt_late)")]
fn end_to_end_when_date_and_time_after_2038_stored_into_ldt_then_zero_extended(#[case] expr: &str) {
    assert_result("LDATE_AND_TIME", expr, LATE);
}

#[rstest]
#[case::variable("d_late")]
#[case::literal("D#2100-01-01")]
#[case::dt_to_date("DT_TO_DATE(dt_late)")]
#[case::conversion("DATE_TO_LDATE(d_late)")]
fn end_to_end_when_date_after_2038_stored_into_ldate_then_zero_extended(#[case] expr: &str) {
    assert_result("LDATE", expr, LATE);
}

/// A `TIME_OF_DAY` never reaches 2^31 (its last value is 86_399_999 ms), so
/// its top bit is never set; this pins that every spelling still stores it
/// unchanged.
#[rstest]
#[case::variable("tod_last", 86_399_000)]
#[case::dt_to_tod("DT_TO_TOD(dt_late)", 0)]
#[case::conversion("TOD_TO_LTOD(tod_last)", 86_399_000)]
fn end_to_end_when_time_of_day_stored_into_ltod_then_value_kept(
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_result("LTIME_OF_DAY", expr, expected);
}

/// A duration is signed (ADR-0021), so a negative `TIME` stays negative.
#[rstest]
#[case::variable("t_neg")]
#[case::conversion("TIME_TO_LTIME(t_neg)")]
fn end_to_end_when_negative_time_stored_into_ltime_then_sign_extended(#[case] expr: &str) {
    assert_result("LTIME", expr, -5_000);
}

/// A short date compared with a long one is widened before the comparison.
#[rstest]
#[case::greater("ldt_hour_later > dt_late", 1)]
#[case::equal("ldt_hour_later = dt_late + T#1h", 1)]
fn end_to_end_when_ldt_compared_with_date_and_time_after_2038_then_zero_extended(
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_result("BOOL", expr, expected);
}

/// The rule is on signedness, not on the temporal types: an unsigned 32-bit
/// function result is widened the way an unsigned variable already is.
#[test]
fn end_to_end_when_udint_function_result_stored_into_lint_then_zero_extended() {
    assert_result("LINT", "late_udint()", 4_000_000_000);
}
