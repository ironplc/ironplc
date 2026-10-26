//! Tests for the conversion the pass records for a time or date stored in a
//! target of the other width of its family: a variable, a literal, and the
//! function form of an arithmetic operator that computes as a typed time or
//! date function. A bit string literal, which has a type of its own as a
//! time or date literal does, is tested with them.

use ironplc_parser::options::Dialect;

use super::*;

/// The value of `x := <value>` for an `x` of type `target`, in a program of
/// the third edition declaring a variable of each temporal type.
fn stored(target: &str, value: &str) -> String {
    let source = format!(
        "PROGRAM main
         VAR x : {target}; t : TIME; lt : LTIME; da : DATE; lda : LDATE; tod1 : TOD;
             dt1 : DT; dw : DWORD; END_VAR
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

#[spec_test(REQ_IC_analyzer_127)]
#[rstest]
#[case::date_and_time("LDT", "DT#2020-01-01-00:00:00", "DATE_AND_TIME->LDATE_AND_TIME")]
#[case::date("LDATE", "D#2020-01-01", "DATE->LDATE")]
#[case::time("LTIME", "T#-5s", "TIME->LTIME")]
#[case::time_of_day("LTOD", "TOD#12:00:00", "TIME_OF_DAY->LTIME_OF_DAY")]
#[case::bit_string("LWORD", "DWORD#16#FF", "DWORD->LWORD")]
#[case::same_width("TIME", "T#5s", "TIME")]
#[case::long_into_long("LDT", "LDT#2020-01-01-00:00:00", "LDATE_AND_TIME")]
#[case::bit_string_of_same_operation_width("DWORD", "WORD#16#FF", "WORD")]
fn apply_when_time_or_date_literal_stored_in_other_width_then_converted(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(stored(target, value), expected);
}

#[spec_test(REQ_IC_analyzer_128)]
#[rstest]
#[case::date_and_time_plus_time("LDT", "ADD(dt1, t)", "DATE_AND_TIME->LDATE_AND_TIME")]
#[case::date_and_time_minus_time("LDT", "SUB(dt1, t)", "DATE_AND_TIME->LDATE_AND_TIME")]
#[case::time_plus_time("LTIME", "ADD(t, t)", "TIME->LTIME")]
#[case::date_minus_date("LTIME", "SUB(da, da)", "TIME->LTIME")]
#[case::time_scaled("LTIME", "MUL(t, 2)", "TIME->LTIME")]
#[case::three_inputs("LTIME", "ADD(t, t, t)", "TIME->LTIME")]
#[case::same_width("TIME", "ADD(t, t)", "TIME")]
fn apply_when_typed_function_form_stored_in_other_width_then_converted(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(stored(target, value), expected);
}
