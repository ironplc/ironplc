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
