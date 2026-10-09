//! Tests for the type the pass records for a call whose result is an
//! integer of its context's type (`TRUNC`, `BCD_TO_INT`, `SIZEOF`), and for
//! `INT_TO_BCD`, whose result is the bit string as wide as its input.

use super::*;

/// The variables the programs of these tests declare.
const VARS: &str = "r : REAL; lr : LREAL; i : INT; d : DINT; l : LINT; b : BYTE; w : WORD;
    dw : DWORD; lw : LWORD;";

/// The value of `x := <value>` for an `x` of type `target`, as [`describe`]
/// shows it.
fn assigned(target: &str, value: &str) -> String {
    let values = assigned_values(&arithmetic_program(target, VARS, value));
    values[0].clone()
}

#[spec_test(REQ_IC_analyzer_088)]
#[rstest]
#[case::trunc_to_lint("LINT", "TRUNC(lr)", "LINT")]
#[case::trunc_to_int("INT", "TRUNC(r)", "INT")]
#[case::bcd_to_int("DINT", "BCD_TO_INT(w)", "DINT")]
#[case::bcd_to_lint("LINT", "BCD_TO_INT(lw)", "LINT")]
fn apply_when_integer_result_in_integer_context_then_takes_context_type(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(assigned(target, value), expected);
}

#[spec_test(REQ_IC_analyzer_089)]
#[rstest]
#[case::trunc_to_real("REAL", "TRUNC(r)", "DINT->REAL")]
#[case::trunc_to_lreal("LREAL", "TRUNC(lr)", "DINT->LREAL")]
#[case::bcd_to_real("REAL", "BCD_TO_INT(w)", "DINT->REAL")]
fn apply_when_integer_result_in_real_context_then_dint_converted(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(assigned(target, value), expected);
}

#[spec_test(REQ_IC_analyzer_089)]
#[test]
fn apply_when_integer_result_compared_with_real_then_dint_converted() {
    let source = program("r : REAL;", "TRUNC(r) < r");
    assert_eq!(comparison_operands(&source), pair("DINT->REAL", "REAL"));
}

#[spec_test(REQ_IC_analyzer_089)]
#[test]
fn apply_when_integer_result_operand_of_real_arithmetic_then_dint_converted() {
    let source = arithmetic_program("REAL", VARS, "TRUNC(r) * r");
    assert_eq!(
        arithmetic_operands(&source),
        operands(&[&["DINT->REAL", "REAL"]])
    );
}

#[spec_test(REQ_IC_analyzer_089)]
#[test]
fn apply_when_integer_result_passed_to_real_parameter_then_dint_converted() {
    let source = call_program("REAL", "y : REAL;", "TRUNC(y)");
    assert_eq!(call_arguments(&source, "f"), vec!["DINT->REAL"]);
}

#[spec_test(REQ_IC_analyzer_090)]
#[rstest]
#[case::of_int("WORD", "INT_TO_BCD(i)", "WORD")]
#[case::of_lint("LWORD", "INT_TO_BCD(l)", "LWORD")]
#[case::of_literal("DWORD", "INT_TO_BCD(42)", "DWORD")]
#[case::widened("LWORD", "INT_TO_BCD(i)", "WORD->LWORD")]
fn apply_when_int_to_bcd_then_bit_string_as_wide_as_input(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(assigned(target, value), expected);
}
