//! Spec conformance tests for the error type of an arithmetic expression
//! that no overload applies to: what has it, and which checks skip it so
//! the expression is reported once.
//!
//! See *Type resolution* in `specs/design/arithmetic-operator-overloads.md`.

use ironplc_dsl::common::TypeName;
use ironplc_dsl::textual::ExprType;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;
use spec_test_macro::spec_test;

use super::{assigned_type, assigned_values, codes, diagnostics, p4049, program};

/// Variables of every type the cases below use.
const VARS: &str = "s1 : STRING; s2 : STRING; d : DINT; r : REAL; b : BOOL;";

/// The type of the value assigned in `program(VARS, result_type, expr)`.
fn value_type_of(result_type: &str, expr: &str) -> Option<ExprType> {
    let values = assigned_values(
        &program(VARS, result_type, expr),
        &CompilerOptions::default(),
    );
    values[0].expr_type.clone()
}

/// The problem codes `program(VARS, result_type, expr)` is reported with.
fn codes_of(result_type: &str, expr: &str) -> Vec<String> {
    codes(&diagnostics(
        &program(VARS, result_type, expr),
        &CompilerOptions::default(),
    ))
}

/// REQ-AO-analyzer-025: an expression or call that does not resolve has the
/// error type.
#[spec_test(REQ_AO_analyzer_025)]
#[rstest]
#[case("STRING", "s1 + s2")]
#[case("REAL", "d * r")]
#[case("BOOL", "b * b")]
#[case("STRING", "ADD(s1, s2)")]
#[case("REAL", "MUL(d, r)")]
fn analyzer_spec_req_ao_025_unresolved_expression_has_error_type(
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    assert_eq!(
        value_type_of(result_type, expr),
        Some(ExprType::Error),
        "{expr}"
    );
}

/// REQ-AO-analyzer-026: an operator, a parenthesised expression or an
/// overloaded call with an operand of the error type has the error type.
#[spec_test(REQ_AO_analyzer_026)]
#[rstest]
#[case("STRING", "(s1 + s2) * 2")]
#[case("STRING", "2 * (s1 + s2)")]
#[case("REAL", "r + d * r")]
#[case("REAL", "(d * r)")]
#[case("REAL", "-(d * r)")]
#[case("BOOL", "NOT (b * b)")]
#[case("BOOL", "(b * b) AND b")]
#[case("BOOL", "b OR (b * b)")]
#[case("REAL", "ADD(d * r, r)")]
#[case("REAL", "ADD(r, d * r)")]
fn analyzer_spec_req_ao_026_expression_with_error_operand_has_error_type(
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    assert_eq!(
        value_type_of(result_type, expr),
        Some(ExprType::Error),
        "{expr}"
    );
}

/// REQ-AO-analyzer-027: a generic function whose result binds to an
/// argument of the error type has the error type.
#[spec_test(REQ_AO_analyzer_027)]
#[rstest]
#[case("REAL", "ABS(d * r)")]
#[case("REAL", "MAX(d * r, 1)")]
#[case("REAL", "MAX(1, d * r)")]
fn analyzer_spec_req_ao_027_generic_call_bound_to_error_has_error_type(
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    assert_eq!(
        value_type_of(result_type, expr),
        Some(ExprType::Error),
        "{expr}"
    );
}

/// REQ-AO-analyzer-028: a comparison with an operand of the error type is
/// `BOOL`, and a function with a concrete return type keeps it.
#[spec_test(REQ_AO_analyzer_028)]
#[rstest]
#[case("BOOL", "(d * r) > 1.0", "BOOL")]
#[case("BOOL", "(s1 + s2) = s1", "BOOL")]
#[case("DINT", "REAL_TO_DINT(d * r)", "DINT")]
fn analyzer_spec_req_ao_028_enclosing_expression_with_known_type_keeps_it(
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        assigned_type(
            &program(VARS, result_type, expr),
            &CompilerOptions::default()
        ),
        Some(TypeName::from(expected)),
        "{expr}"
    );
}

/// REQ-AO-analyzer-037: an operator or overloaded call with an operand of
/// the error type is not reported again.
#[spec_test(REQ_AO_analyzer_037)]
#[rstest]
#[case("STRING", "(s1 + s2) * 2")]
#[case("STRING", "2 * (s1 + s2)")]
#[case("REAL", "ADD(d * r, r)")]
#[case("REAL", "MUL(r, d * r)")]
#[case("BOOL", "(s1 + s2) AND b")]
#[case("BOOL", "NOT (s1 + s2)")]
fn analyzer_spec_req_ao_037_enclosing_operator_is_not_reported(
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    assert_eq!(codes_of(result_type, expr), vec![p4049()], "{expr}");
}

/// REQ-AO-analyzer-038: an assignment of a value of the error type is not
/// reported, so there is no P4035.
#[spec_test(REQ_AO_analyzer_038)]
#[rstest]
#[case("REAL", "d * r")]
#[case("DINT", "s1 + s2")]
#[case("DINT", "(s1 + s2) * 2")]
#[case("DINT", "-(s1 + s2)")]
fn analyzer_spec_req_ao_038_assignment_of_error_is_not_reported(
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    assert_eq!(codes_of(result_type, expr), vec![p4049()], "{expr}");
}

/// REQ-AO-analyzer-039: a call whose result has the error type is not
/// reported against its assignment target, so there is no P4027.
#[spec_test(REQ_AO_analyzer_039)]
#[rstest]
#[case("DINT", "ADD(s1, s2)")]
#[case("REAL", "ADD(d * r, r)")]
#[case("DINT", "MAX(1, s1 + s2)")]
fn analyzer_spec_req_ao_039_call_returning_error_is_not_reported(
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    assert_eq!(codes_of(result_type, expr), vec![p4049()], "{expr}");
}

/// REQ-AO-analyzer-040: a call argument of the error type is not reported,
/// so there is no P4026.
#[spec_test(REQ_AO_analyzer_040)]
#[rstest]
#[case("DINT", "MAX(1, s1 + s2)")]
#[case("DINT", "LIMIT(0, s1 + s2, 10)")]
#[case("DINT", "REAL_TO_DINT(s1 + s2)")]
#[case("STRING", "CONCAT(s1, d * r)")]
fn analyzer_spec_req_ao_040_call_argument_of_error_is_not_reported(
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    assert_eq!(codes_of(result_type, expr), vec![p4049()], "{expr}");
}

/// REQ-AO-analyzer-041: a `CASE` selector or a condition of the error type
/// is not reported.
#[spec_test(REQ_AO_analyzer_041)]
#[rstest]
#[case("CASE s1 + s2 OF 1: d := 1; END_CASE;")]
#[case("IF s1 + s2 THEN d := 1; END_IF;")]
#[case("IF b THEN d := 1; ELSIF d * r THEN d := 2; END_IF;")]
#[case("WHILE (d * r) > 1.0 DO d := 1; END_WHILE;")]
#[case("REPEAT d := 1; UNTIL s1 + s2 END_REPEAT;")]
fn analyzer_spec_req_ao_041_selector_or_condition_of_error_is_not_reported(
    #[case] statement: &str,
) {
    let source = format!(
        "PROGRAM main
VAR
    {VARS}
END_VAR
    {statement}
END_PROGRAM"
    );
    assert_eq!(
        codes(&diagnostics(&source, &CompilerOptions::default())),
        vec![p4049()],
        "{statement}"
    );
}
