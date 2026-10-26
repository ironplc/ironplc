//! Tests for the conversions the pass records for a function of several
//! inputs of one type: `MIN`, `MAX`, `LIMIT`, `SEL`, `MUX`, `EXPT`, `ATAN2`.

use super::*;

/// The variables the programs of these tests declare.
const VARS: &str = "s : SINT; i : INT; d : DINT; e : DINT; l : LINT; k : LINT; n : INT;
    us : USINT; ui : UINT; r : REAL; lr : LREAL; g : BOOL;";

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
#[case::unsigned_of_result_width("MAX(us, d)", "MAX", &["USINT->DINT", "DINT"])]
#[case::min_widest_first("MIN(l, d)", "MIN", &["LINT", "DINT->LINT"])]
#[case::limit("LIMIT(i, d, l)", "LIMIT", &["INT->LINT", "DINT->LINT", "LINT"])]
#[case::sel("SEL(g, i, l)", "SEL", &["BOOL", "INT->LINT", "LINT"])]
#[case::mux("MUX(n, i, l)", "MUX", &["INT", "INT->LINT", "LINT"])]
#[case::atan2("ATAN2(r, lr)", "ATAN2", &["REAL->LREAL", "LREAL"])]
#[case::expt_exponent("EXPT(r, l)", "EXPT", &["REAL", "LINT->REAL"])]
#[case::real_literal_with_narrow_integer("MIN(i, 1.5)", "MIN", &["INT->REAL", "REAL"])]
fn apply_when_input_of_another_operation_type_then_converted_to_result_type(
    #[case] value: &str,
    #[case] name: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(inputs("LREAL", value, name), strings(expected));
}

#[spec_test(REQ_IC_analyzer_074)]
#[rstest]
#[case::signed("INT", "MAX(s, i)", &["SINT", "INT"])]
#[case::unsigned("UINT", "MAX(us, ui)", &["USINT", "UINT"])]
fn apply_when_inputs_share_result_operation_type_then_unchanged(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(inputs(target, value, "MAX"), strings(expected));
}

#[spec_test(REQ_IC_analyzer_075)]
#[rstest]
#[case::wide_mux_selector("MUX(k, d, e)", "MUX", &["LINT->DINT", "DINT", "DINT"])]
#[case::narrow_mux_selector("MUX(n, d, e)", "MUX", &["INT", "DINT", "DINT"])]
#[case::unsigned_mux_selector("MUX(us, d, e)", "MUX", &["USINT->DINT", "DINT", "DINT"])]
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

/// The inputs `SEL` and `MUX` select between widen to one type whatever
/// their type, as a number's do: the short one of a time or date or a bit
/// string beside the long one is converted to it.
#[spec_test(REQ_IC_analyzer_129)]
#[rstest]
#[case::sel_dates("LDATE", "SEL(g, da, lda)", "SEL", &["BOOL", "DATE->LDATE", "LDATE"])]
#[case::mux_durations("LTIME", "MUX(n, lt, t)", "MUX", &["INT", "LTIME", "TIME->LTIME"])]
#[case::sel_bit_strings("LWORD", "SEL(g, w, lw)", "SEL", &["BOOL", "WORD->LWORD", "LWORD"])]
fn apply_when_selected_inputs_of_another_width_then_converted_to_result_type(
    #[case] target: &str,
    #[case] value: &str,
    #[case] name: &str,
    #[case] expected: &[&str],
) {
    let vars = "g : BOOL; n : INT; da : DATE; lda : LDATE; t : TIME; lt : LTIME; w : WORD;
        lw : LWORD;";
    let source = arithmetic_program(target, vars, value);
    let options = CompilerOptions::from_dialect(ironplc_parser::options::Dialect::Iec61131_3Ed3);
    assert_eq!(
        super::call_arguments::call_arguments_with(&source, name, &options),
        strings(expected)
    );
}
