//! Spec conformance tests for the function forms of operators (analyzer-owned
//! requirements).
//!
//! Each test is annotated with `#[spec_test(REQ_KF_analyzer_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every analyzer-owned requirement has a test.
//!
//! See `specs/design/keyword-function-forms.md`.

use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName};
use ironplc_dsl::core::{FileId, Id};
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::intermediates::operator_function_form::operator_function_form;
use crate::stages::analyze;

/// Analyzes `program` under default options and returns its problem codes.
fn analyze_codes(program: &str) -> Vec<String> {
    let library = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (_library, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
    context
        .diagnostics()
        .iter()
        .map(|d| d.code.clone())
        .collect()
}

/// A program that assigns `expr`, over operands `a` and `b` of `operand_type`,
/// to `result` of `result_type`.
fn program(operand_type: &str, result_type: &str, expr: &str) -> String {
    format!(
        "PROGRAM main
VAR
    a : {operand_type};
    b : {operand_type};
    result : {result_type};
END_VAR
    result := {expr};
END_PROGRAM"
    )
}

/// The call expression for the function form `function`: the unary spelling
/// for `NOT`, which has to be named because `NOT(a)` parses as the operator.
fn call(function: &str) -> String {
    if function == "NOT" {
        "NOT(IN := a)".to_string()
    } else {
        format!("{function}(a, b)")
    }
}

fn assert_clean(operand_type: &str, result_type: &str, expr: &str) {
    let codes = analyze_codes(&program(operand_type, result_type, expr));
    assert!(
        codes.is_empty(),
        "{expr} on {operand_type} into {result_type}: expected clean analysis, got {codes:?}"
    );
}

/// REQ-KF-analyzer-001: the arithmetic forms accept every ANY_NUM type and
/// return the operand type, so the result assigns to a variable of that type.
#[spec_test(REQ_KF_analyzer_001)]
#[rstest]
fn analyzer_spec_req_kf_001_arithmetic_forms_accept_any_num(
    #[values("ADD", "SUB", "MUL", "DIV", "MOD")] function: &str,
    #[values(
        "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "REAL", "LREAL"
    )]
    operand_type: &str,
) {
    assert_clean(operand_type, operand_type, &call(function));
}

/// REQ-KF-analyzer-002: the comparison forms accept every ANY_ELEMENTARY type
/// and return BOOL.
#[spec_test(REQ_KF_analyzer_002)]
#[rstest]
fn analyzer_spec_req_kf_002_comparison_forms_accept_any_elementary(
    #[values("GT", "GE", "EQ", "LE", "LT", "NE")] function: &str,
    #[values(
        "BOOL",
        "SINT",
        "INT",
        "DINT",
        "LINT",
        "USINT",
        "UINT",
        "UDINT",
        "ULINT",
        "REAL",
        "LREAL",
        "BYTE",
        "WORD",
        "DWORD",
        "LWORD",
        "STRING",
        "WSTRING",
        "TIME",
        "LTIME",
        "DATE",
        "LDATE",
        "TIME_OF_DAY",
        "LTIME_OF_DAY",
        "DATE_AND_TIME",
        "LDATE_AND_TIME"
    )]
    operand_type: &str,
) {
    assert_clean(operand_type, "BOOL", &call(function));
}

/// REQ-KF-analyzer-003: AND, OR and XOR accept every ANY_BIT type and return
/// the operand type.
#[spec_test(REQ_KF_analyzer_003)]
#[rstest]
fn analyzer_spec_req_kf_003_bitwise_forms_accept_any_bit(
    #[values("AND", "OR", "XOR")] function: &str,
    #[values("BOOL", "BYTE", "WORD", "DWORD", "LWORD")] operand_type: &str,
) {
    assert_clean(operand_type, operand_type, &call(function));
}

/// REQ-KF-analyzer-004: NOT accepts every ANY_BIT type and returns the operand
/// type.
#[spec_test(REQ_KF_analyzer_004)]
#[rstest]
fn analyzer_spec_req_kf_004_not_form_accepts_any_bit(
    #[values("BOOL", "BYTE", "WORD", "DWORD", "LWORD")] operand_type: &str,
) {
    assert_clean(operand_type, operand_type, &call("NOT"));
}

/// REQ-KF-analyzer-005: an argument outside the operand category is P4026.
///
/// Every function form against every elementary type. Which side of the line
/// a type falls on comes from the form's own row and the DSL's definition of
/// the category, not from a list written here, so the matrix is complete by
/// construction: a type the category admits must analyze clean, and every
/// other type must be P4026.
#[spec_test(REQ_KF_analyzer_005)]
#[rstest]
fn analyzer_spec_req_kf_005_argument_outside_category_is_p4026(
    #[values(
        "ADD", "SUB", "MUL", "DIV", "MOD", "GT", "GE", "EQ", "LE", "LT", "NE", "AND", "OR", "XOR",
        "NOT"
    )]
    function: &str,
    #[values(
        "BOOL",
        "SINT",
        "INT",
        "DINT",
        "LINT",
        "USINT",
        "UINT",
        "UDINT",
        "ULINT",
        "REAL",
        "LREAL",
        "BYTE",
        "WORD",
        "DWORD",
        "LWORD",
        "STRING",
        "WSTRING",
        "TIME",
        "LTIME",
        "DATE",
        "LDATE",
        "TIME_OF_DAY",
        "LTIME_OF_DAY",
        "DATE_AND_TIME",
        "LDATE_AND_TIME"
    )]
    operand_type: &str,
) {
    let signature = operator_function_form(function).unwrap().signature();
    let category = GenericTypeName::try_from(&signature.parameters[0].param_type.name).unwrap();
    let result_type = signature.return_type.unwrap().to_type_name();
    let result_type = if GenericTypeName::try_from(&result_type.name).is_ok() {
        operand_type
    } else {
        "BOOL"
    };
    let elementary = ElementaryTypeName::try_from(&Id::from(operand_type)).unwrap();
    let admitted = category.is_compatible_with(&elementary);

    let expr = call(function);
    let codes = analyze_codes(&program(operand_type, result_type, &expr));
    let p4026 = Problem::FunctionCallArgTypeMismatch.code().to_string();
    if admitted {
        assert!(
            codes.is_empty(),
            "{expr} on {operand_type}: {category:?} admits it, expected clean analysis, got {codes:?}"
        );
    } else {
        assert!(
            codes.contains(&p4026),
            "{expr} on {operand_type}: outside {category:?}, expected {p4026}, got {codes:?}"
        );
    }
}
