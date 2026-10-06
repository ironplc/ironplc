//! End-to-end tests for the long forms of the typed time and date functions
//! (`ADD_LTIME`, `SUB_LDATE_LDATE`, `MUL_LTIME`, ...).
//!
//! A long form is its short form's sequence at 64-bit width over the long
//! types, which store the same units as the short ones (ADR-0021,
//! ADR-0025): durations and times of day in milliseconds, dates in seconds.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::{
    assert_run_with, datetime, time, Duration, FromValue, PrimitiveDateTime, Time,
};

/// A program assigning `expr` to `result` of `result_type`, with operands of
/// every long and short temporal type.
fn program(result_type: &str, expr: &str) -> String {
    format!(
        "
PROGRAM main
  VAR
    result : {result_type};
    lt1 : LTIME := LTIME#2h;
    lt2 : LTIME := LTIME#30m;
    ltod1 : LTIME_OF_DAY := LTOD#10:00:00;
    ltod2 : LTIME_OF_DAY := LTOD#08:30:00;
    ld1 : LDATE := LDATE#2000-01-02;
    ld2 : LDATE := LDATE#2000-01-01;
    ldt1 : LDATE_AND_TIME := LDT#2000-01-01-01:00:00;
    ldt2 : LDATE_AND_TIME := LDT#2000-01-01-00:00:00;
    ldt_late : LDATE_AND_TIME := LDT#2100-01-01-01:00:00;
    dt_late : DATE_AND_TIME := DT#2100-01-01-00:00:00;
    t : TIME := T#-5s;
    d : DINT := 3;
    u : UDINT := 4;
    l : LINT := 4;
    r : REAL := 1.5;
    lr : LREAL := 2.5;
  END_VAR
  result := {expr};
END_PROGRAM
"
    )
}

/// Asserts that `result`, of `result_type`, holds `expected` after `expr`.
fn assert_long<T: FromValue>(result_type: &str, expr: &str, expected: T) {
    assert_run_with(
        &program(result_type, expr),
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[("result", expected)],
    );
}

#[rstest]
#[case::add_ltime("ADD_LTIME(lt1, lt2)", Duration::minutes(150))]
// 60 days does not fit a 32-bit TIME.
#[case::add_ltime_beyond_32_bits("ADD_LTIME(LTIME#30d, LTIME#30d)", Duration::days(60))]
// A short TIME operand is sign-extended: 2h + (-5s).
#[case::add_ltime_short_negative_operand(
    "ADD_LTIME(lt1, t)",
    Duration::hours(2) - Duration::seconds(5)
)]
#[case::sub_ltime("SUB_LTIME(lt1, lt2)", Duration::minutes(90))]
#[case::mul_ltime_by_dint("MUL_LTIME(lt1, d)", Duration::hours(6))]
#[case::mul_ltime_by_udint("MUL_LTIME(lt1, u)", Duration::hours(8))]
#[case::mul_ltime_by_lint("MUL_LTIME(lt1, l)", Duration::hours(8))]
#[case::mul_ltime_by_real("MUL_LTIME(lt1, r)", Duration::hours(3))]
#[case::mul_ltime_by_lreal("MUL_LTIME(lt1, lr)", Duration::hours(5))]
#[case::mul_ltime_beyond_32_bits("MUL_LTIME(LTIME#30d, d)", Duration::days(90))]
#[case::div_ltime_by_dint("DIV_LTIME(lt1, d)", Duration::minutes(40))]
#[case::div_ltime_by_real("DIV_LTIME(lt1, r)", Duration::minutes(80))]
#[case::sub_ltod_ltod("SUB_LTOD_LTOD(ltod1, ltod2)", Duration::minutes(90))]
#[case::sub_ldt_ldt("SUB_LDT_LDT(ldt1, ldt2)", Duration::hours(1))]
#[case::sub_ldate_ldate("SUB_LDATE_LDATE(ld1, ld2)", Duration::days(1))]
// A short DATE_AND_TIME past 2038 does not fit in an i32, so it must be
// zero-extended, not sign-extended, to meet a long operand.
#[case::sub_ldt_ldt_short_operand_after_2038("SUB_LDT_LDT(ldt_late, dt_late)", Duration::hours(1))]
fn end_to_end_when_long_typed_time_function_then_computes_at_64_bits(
    #[case] expr: &str,
    #[case] expected: Duration,
) {
    assert_long("LTIME", expr, expected);
}

#[rstest]
#[case::add_ltod_ltime("ADD_LTOD_LTIME(ltod1, lt2)", time!(10:30))]
#[case::sub_ltod_ltime("SUB_LTOD_LTIME(ltod1, lt2)", time!(9:30))]
fn end_to_end_when_long_time_of_day_function_then_computes_at_64_bits(
    #[case] expr: &str,
    #[case] expected: Time,
) {
    assert_long("LTIME_OF_DAY", expr, expected);
}

#[rstest]
#[case::add_ldt_ltime("ADD_LDT_LTIME(ldt2, LTIME#1h)", datetime!(2000-01-01 1:00))]
#[case::sub_ldt_ltime("SUB_LDT_LTIME(ldt1, LTIME#1h)", datetime!(2000-01-01 0:00))]
fn end_to_end_when_long_date_and_time_function_then_computes_at_64_bits(
    #[case] expr: &str,
    #[case] expected: PrimitiveDateTime,
) {
    assert_long("LDATE_AND_TIME", expr, expected);
}
