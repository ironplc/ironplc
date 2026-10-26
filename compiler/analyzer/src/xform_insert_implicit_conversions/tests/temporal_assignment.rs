//! Tests for the conversion the pass records for a time or date stored in a
//! target of the other width of its family.

use ironplc_parser::options::Dialect;

use super::*;

/// The value of `x := <value>` for an `x` of type `target`, in a program of
/// the third edition declaring a variable of each temporal type.
fn stored(target: &str, value: &str) -> String {
    let source = format!(
        "PROGRAM main
         VAR x : {target}; t : TIME; lt : LTIME; da : DATE; lda : LDATE; tod1 : TOD;
             dt1 : DT; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    assigned_values_with(&source, &options)[0].clone()
}

#[spec_test(REQ_IC_analyzer_099)]
#[rstest]
#[case::time("LTIME", "t", "TIME->LTIME")]
#[case::date("LDATE", "da", "DATE->LDATE")]
#[case::time_of_day("LTOD", "tod1", "TIME_OF_DAY->LTIME_OF_DAY")]
#[case::date_and_time("LDT", "dt1", "DATE_AND_TIME->LDATE_AND_TIME")]
#[case::same_width("TIME", "t", "TIME")]
#[case::long_into_long("LDATE", "lda", "LDATE")]
fn apply_when_time_or_date_stored_in_other_width_then_converted(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(stored(target, value), expected);
}

/// A time or date literal has the type its prefix names, and converts to
/// the target's as a variable of that type does.
#[spec_test(REQ_IC_analyzer_127)]
#[rstest]
#[case::time("LTIME", "T#-5s", "TIME->LTIME")]
#[case::date("LDATE", "D#2100-01-01", "DATE->LDATE")]
#[case::time_of_day("LTOD", "TOD#23:00:00", "TIME_OF_DAY->LTIME_OF_DAY")]
#[case::date_and_time("LDT", "DT#2100-01-01-00:00:00", "DATE_AND_TIME->LDATE_AND_TIME")]
#[case::same_width("TIME", "T#5s", "TIME")]
#[case::long_into_long("LDT", "LDT#2100-01-01-00:00:00", "LDATE_AND_TIME")]
fn apply_when_time_or_date_literal_stored_in_other_width_then_converted(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(stored(target, value), expected);
}

/// The function form of an operator on time or date operands computes as
/// the typed function, and its result converts as the typed call's does.
#[spec_test(REQ_IC_analyzer_128)]
#[rstest]
#[case::add_dt_time("LDT", "ADD(dt1, t)", "DATE_AND_TIME->LDATE_AND_TIME")]
#[case::sub_dt_time("LDT", "SUB(dt1, t)", "DATE_AND_TIME->LDATE_AND_TIME")]
#[case::add_tod_time("LTOD", "ADD(tod1, t)", "TIME_OF_DAY->LTIME_OF_DAY")]
#[case::add_time("LTIME", "ADD(t, t)", "TIME->LTIME")]
#[case::sub_dt_dt("LTIME", "SUB(dt1, dt1)", "TIME->LTIME")]
#[case::mul_time("LTIME", "MUL(t, 2)", "TIME->LTIME")]
#[case::same_width("TIME", "ADD(t, t)", "TIME")]
#[case::long_form("LTIME", "ADD(lt, t)", "LTIME")]
fn apply_when_function_form_on_time_or_date_stored_in_other_width_then_result_converted(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(stored(target, value), expected);
}
