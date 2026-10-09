//! Tests for the conversions the pass records for a function of several
//! inputs of one type: `MIN`, `MAX`, `LIMIT`, `SEL`, `MUX`, `EXPT`, `ATAN2`.

use super::*;

/// The variables the programs of these tests declare.
const VARS: &str = "s : SINT; i : INT; d : DINT; e : DINT; l : LINT; k : LINT; n : INT;
    r : REAL; lr : LREAL; g : BOOL;";

/// The inputs of every call to `name` in a program assigning `value` to a
/// variable of type `target`, each as [`describe`] shows it.
fn inputs(target: &str, value: &str, name: &str) -> Vec<String> {
    call_arguments(&arithmetic_program(target, VARS, value), name)
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[spec_test(REQ_IC_analyzer_073)]
#[rstest]
#[case::max("MAX(i, l)", "MAX", &["INT->LINT", "LINT"])]
#[case::min_widest_first("MIN(l, d)", "MIN", &["LINT", "DINT->LINT"])]
#[case::limit("LIMIT(i, d, l)", "LIMIT", &["INT->LINT", "DINT->LINT", "LINT"])]
#[case::sel("SEL(g, i, l)", "SEL", &["BOOL", "INT->LINT", "LINT"])]
#[case::mux("MUX(n, i, l)", "MUX", &["INT", "INT->LINT", "LINT"])]
#[case::atan2("ATAN2(r, lr)", "ATAN2", &["REAL->LREAL", "LREAL"])]
#[case::expt_exponent("EXPT(r, l)", "EXPT", &["REAL", "LINT->REAL"])]
fn apply_when_input_of_another_width_then_converted_to_result_type(
    #[case] value: &str,
    #[case] name: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(inputs("LREAL", value, name), strings(expected));
}

#[spec_test(REQ_IC_analyzer_074)]
#[test]
fn apply_when_inputs_share_result_width_then_unchanged() {
    assert_eq!(inputs("INT", "MAX(s, i)", "MAX"), strings(&["SINT", "INT"]));
}

#[spec_test(REQ_IC_analyzer_075)]
#[rstest]
#[case::wide_mux_selector("MUX(k, d, e)", "MUX", &["LINT->DINT", "DINT", "DINT"])]
#[case::narrow_mux_selector("MUX(n, d, e)", "MUX", &["INT", "DINT", "DINT"])]
#[case::sel_selector("SEL(g, d, e)", "SEL", &["BOOL", "DINT", "DINT"])]
fn apply_when_selector_then_converted_to_dint_or_left_a_bool(
    #[case] value: &str,
    #[case] name: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(inputs("DINT", value, name), strings(expected));
}

#[spec_test(REQ_IC_analyzer_076)]
#[test]
fn apply_when_result_is_literal_category_then_takes_context_type() {
    let source = arithmetic_program("LINT", VARS, "MAX(1, 2)");
    assert_eq!(assigned_values(&source), strings(&["LINT"]));
    assert_eq!(call_arguments(&source, "MAX"), strings(&["LINT", "LINT"]));

    let source = arithmetic_program("LREAL", VARS, "EXPT(2, r)");
    assert_eq!(assigned_values(&source), strings(&["LREAL"]));
    assert_eq!(
        call_arguments(&source, "EXPT"),
        strings(&["LREAL", "REAL->LREAL"])
    );
}

#[spec_test(REQ_IC_analyzer_077)]
#[test]
fn apply_when_result_assigned_to_wider_target_then_converted() {
    let source = arithmetic_program("LINT", VARS, "MAX(d, e)");
    assert_eq!(assigned_values(&source), strings(&["DINT->LINT"]));
}

#[test]
fn apply_when_result_compared_with_wider_operand_then_converted() {
    let source = program("d : DINT; e : DINT; l : LINT;", "MAX(d, e) < l");
    // The visitor reports the inputs of every call of two inputs too.
    assert_eq!(
        comparison_operands(&source),
        vec![
            ["DINT->LINT".to_string(), "LINT".to_string()],
            ["DINT".to_string(), "DINT".to_string()]
        ]
    );
}

#[test]
fn apply_when_result_is_operand_of_wider_arithmetic_then_converted() {
    let source = arithmetic_program("LINT", VARS, "MAX(d, e) + l");
    assert_eq!(
        arithmetic_operands(&source),
        operands(&[&["DINT->LINT", "LINT"]])
    );
}

#[test]
fn apply_when_result_is_input_of_wider_call_then_converted() {
    assert_eq!(
        inputs("LINT", "MAX(MIN(i, d), l)", "MAX"),
        strings(&["DINT->LINT", "LINT"])
    );
}

#[test]
fn apply_when_result_passed_to_wider_parameter_then_converted() {
    let source = call_program("LINT", "d : DINT; e : DINT;", "MAX(d, e)");
    assert_eq!(call_arguments(&source, "f"), strings(&["DINT->LINT"]));
}
