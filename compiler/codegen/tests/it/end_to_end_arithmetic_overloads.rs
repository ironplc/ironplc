//! End-to-end tests for the arithmetic operators on the time and date
//! types: an operator on a Table 30 pair computes what its typed function
//! computes (`REQ-AO-codegen-002` to `-005` and `-009`).
//!
//! These are plain tests named for their requirement until the design is
//! listed in the codegen crate's `build.rs`.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::{assert_run_i32, assert_run_i64_with};

/// A program assigning `expr` to `result` of `result_type`, the first
/// variable (index 0), with short temporal operands.
fn program(result_type: &str, expr: &str) -> String {
    format!(
        "
PROGRAM main
  VAR
    result : {result_type};
    dt1 : DATE_AND_TIME := DT#2000-01-01-00:00:00;
    t1 : TIME := T#1h;
    t2 : TIME := T#1s;
    t3 : TIME := T#30m;
    tod1 : TIME_OF_DAY := TOD#10:00:00;
    d1 : DATE := D#2000-01-02;
    d2 : DATE := D#2000-01-01;
    r : REAL := 1.5;
    d : DINT := 3;
  END_VAR
  result := {expr};
END_PROGRAM
"
    )
}

/// REQ-AO-codegen-002: `dt1 + t1` converts the duration from milliseconds
/// to seconds, as ADD_DT_TIME does. 2000-01-01-00:00:00 is 946684800
/// seconds since the epoch.
#[rstest]
#[case::operator("dt1 + t1")]
#[case::typed("ADD_DT_TIME(dt1, t1)")]
#[case::function_form("ADD(dt1, t1)")]
fn end_to_end_spec_req_ao_002_dt_plus_time_converts_units(#[case] expr: &str) {
    assert_run_i32(&program("DATE_AND_TIME", expr), &[(0, 946_688_400)]);
}

/// REQ-AO-codegen-003: `t * r` promotes to floating point, as MUL_TIME
/// does: 1 s times 1.5 is 1500 ms.
#[rstest]
#[case::operator("t2 * r", 1_500)]
#[case::typed("MUL_TIME(t2, r)", 1_500)]
#[case::function_form("MUL(t2, r)", 1_500)]
#[case::divide("t1 / r", 2_400_000)]
fn end_to_end_spec_req_ao_003_time_times_real_promotes(#[case] expr: &str, #[case] expected: i32) {
    assert_run_i32(&program("TIME", expr), &[(0, expected)]);
}

/// REQ-AO-codegen-004: `d1 - d2` on DATE computes a TIME in milliseconds,
/// as SUB_DATE_DATE does: one day is 86400000 ms.
#[rstest]
#[case::operator("d1 - d2")]
#[case::typed("SUB_DATE_DATE(d1, d2)")]
#[case::function_form("SUB(d1, d2)")]
fn end_to_end_spec_req_ao_004_date_minus_date_is_time_in_millis(#[case] expr: &str) {
    assert_run_i32(&program("TIME", expr), &[(0, 86_400_000)]);
}

/// The other Table 30 rows through the operator, each equal to its typed
/// function's value.
#[rstest]
#[case::add_time("TIME", "t1 + t3", 5_400_000)]
#[case::sub_time("TIME", "t1 - t3", 1_800_000)]
#[case::add_tod_time("TIME_OF_DAY", "tod1 + t3", 37_800_000)]
#[case::sub_tod_time("TIME_OF_DAY", "tod1 - t3", 34_200_000)]
#[case::sub_tod_tod("TIME", "tod1 - TOD#08:30:00", 5_400_000)]
#[case::sub_dt_time("DATE_AND_TIME", "dt1 - t1", 946_681_200)]
#[case::sub_dt_dt("TIME", "DT#2000-01-01-01:00:00 - dt1", 3_600_000)]
#[case::mul_time_dint("TIME", "t2 * d", 3_000)]
#[case::div_time_dint("TIME", "t1 / d", 1_200_000)]
fn end_to_end_when_operator_on_table_30_pair_then_typed_value(
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: i32,
) {
    assert_run_i32(&program(result_type, expr), &[(0, expected)]);
}

/// A program assigning `expr` to `result` of `result_type`, the first
/// variable (index 0), with long and short temporal operands.
fn long_program(result_type: &str, expr: &str) -> String {
    format!(
        "
PROGRAM main
  VAR
    result : {result_type};
    lt1 : LTIME := LTIME#30d;
    lt2 : LTIME := LTIME#2h;
    t : TIME := T#-5s;
    ldt_late : LDATE_AND_TIME := LDT#2100-01-01-01:00:00;
    dt_late : DATE_AND_TIME := DT#2100-01-01-00:00:00;
  END_VAR
  result := {expr};
END_PROGRAM
"
    )
}

/// REQ-AO-codegen-005: the long forms compute at 64 bits, a short TIME
/// operand is sign-extended and a short date operand zero-extended.
#[rstest]
// 60 days in milliseconds does not fit in 32 bits.
#[case::ltime_plus_ltime("lt1 + lt1", 5_184_000_000)]
// 2h + (-5s): the short TIME is sign-extended.
#[case::ltime_plus_negative_time("lt2 + t", 7_195_000)]
#[case::time_plus_ltime("t + lt2", 7_195_000)]
// A DATE_AND_TIME past 2038 does not fit in an i32: zero-extended.
#[case::ldt_minus_dt_after_2038("ldt_late - dt_late", 3_600_000)]
#[case::ltime_times_dint("lt1 * 3", 7_776_000_000)]
fn end_to_end_spec_req_ao_005_long_forms_compute_at_64_bits(
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_run_i64_with(
        &long_program("LTIME", expr),
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(0, expected)],
    );
}

/// REQ-AO-codegen-009: an extensible call on a Table 30 pair computes what
/// the operator chain computes.
#[rstest]
#[case::function_form("ADD(t1, t3, t2)")]
#[case::operator("t1 + t3 + t2")]
fn end_to_end_spec_req_ao_009_extensible_call_folds_from_the_left(#[case] expr: &str) {
    assert_run_i32(&program("TIME", expr), &[(0, 5_401_000)]);
}
