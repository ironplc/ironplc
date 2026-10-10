//! Tests for the conversion the pass records of the result of a standard
//! function whose signature fixes its type: a type conversion, a typed time
//! or date function, `DT_TO_DATE` and a string function.

use ironplc_parser::options::Dialect;

use super::*;

/// The value of `x := <value>` for an `x` of type `target`, in a program of
/// the third edition declaring a variable of each type the tests use.
fn assigned(target: &str, value: &str) -> String {
    let source = format!(
        "PROGRAM main
         VAR x : {target}; i : INT; d : DINT; dt1 : DT; t : TIME; s : STRING; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    assigned_values_with(&source, &options)[0].clone()
}

#[spec_test(REQ_IC_analyzer_102)]
#[rstest]
#[case::conversion_to_real("LREAL", "INT_TO_REAL(i)", "REAL->LREAL")]
#[case::conversion_to_unsigned("LINT", "DINT_TO_UDINT(d)", "UDINT->LINT")]
#[case::time_function("LDT", "SUB_DT_TIME(dt1, t)", "DATE_AND_TIME->LDATE_AND_TIME")]
#[case::date_of("LDATE", "DT_TO_DATE(dt1)", "DATE->LDATE")]
#[case::string_function("LINT", "LEN(s)", "INT->LINT")]
#[case::same_width("DINT", "LEN(s)", "INT")]
fn apply_when_fixed_result_assigned_to_other_width_then_converted(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(assigned(target, value), expected);
}
