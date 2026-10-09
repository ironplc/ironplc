//! Tests for the conversions the pass records for an operand of a subrange
//! type, which is operated at the subrange's base type.

use super::*;

/// The types and variables the programs of these tests declare.
const TYPES: &str = "TYPE Small : INT (-100..100); Big : LINT (0..10000000000);
    Count : UINT (0..60000); END_TYPE";
const VARS: &str = "s : Small; b : Big; c : Count; i : INT; l : LINT;";

/// A program assigning `<expr>` to an `x` of type `target`.
fn subrange_program(target: &str, expr: &str) -> String {
    format!("{TYPES} {}", arithmetic_program(target, VARS, expr))
}

#[spec_test(REQ_IC_analyzer_092)]
#[rstest]
#[case::base_type_operand("INT", "s + i", ["INT", "INT"])]
#[case::wider_subrange_operand("LINT", "s + b", ["INT->LINT", "LINT"])]
#[case::wider_variable_operand("LINT", "s * l", ["INT->LINT", "LINT"])]
fn apply_when_subrange_operand_of_arithmetic_then_operated_at_base_type(
    #[case] target: &str,
    #[case] expr: &str,
    #[case] expected: [&str; 2],
) {
    let source = subrange_program(target, expr);
    assert_eq!(arithmetic_operands(&source), operands(&[&expected]));
}

#[spec_test(REQ_IC_analyzer_092)]
#[rstest]
#[case::arithmetic("DINT", "s + i", "INT")]
#[case::arithmetic_widened("LINT", "s + i", "INT->LINT")]
#[case::wide_arithmetic("LINT", "b + b", "LINT")]
#[case::inputs_of_one_type("LINT", "MAX(c, c)", "UINT->LINT")]
#[case::negation("LINT", "-s", "INT->LINT")]
fn apply_when_operation_on_subrange_assigned_then_result_has_base_type(
    #[case] target: &str,
    #[case] expr: &str,
    #[case] expected: &str,
) {
    let values = assigned_values(&subrange_program(target, expr));
    assert_eq!(values, vec![expected]);
}
