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

use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;

use crate::stages::analyze;

const BIT_STRING_TYPES: &[&str] = &["BOOL", "BYTE", "WORD", "DWORD", "LWORD"];

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

fn assert_clean(operand_type: &str, result_type: &str, expr: &str) {
    let codes = analyze_codes(&program(operand_type, result_type, expr));
    assert!(
        codes.is_empty(),
        "{expr} on {operand_type} into {result_type}: expected clean analysis, got {codes:?}"
    );
}

/// REQ-KF-analyzer-001: the arithmetic forms accept ANY_NUM and return the
/// operand type, so the result assigns to a variable of that type.
#[spec_test(REQ_KF_analyzer_001)]
fn analyzer_spec_req_kf_001_arithmetic_forms_accept_any_num() {
    for function in ["ADD", "SUB", "MUL", "DIV", "MOD"] {
        for operand_type in [
            "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT",
        ] {
            assert_clean(operand_type, operand_type, &format!("{function}(a, b)"));
        }
    }
    for function in ["ADD", "SUB", "MUL", "DIV"] {
        for operand_type in ["REAL", "LREAL"] {
            assert_clean(operand_type, operand_type, &format!("{function}(a, b)"));
        }
    }
}

/// REQ-KF-analyzer-002: the comparison forms accept ANY_ELEMENTARY and return
/// BOOL.
#[spec_test(REQ_KF_analyzer_002)]
fn analyzer_spec_req_kf_002_comparison_forms_accept_any_elementary() {
    for function in ["GT", "GE", "EQ", "LE", "LT", "NE"] {
        for operand_type in ["INT", "DINT", "REAL", "WORD", "TIME"] {
            assert_clean(operand_type, "BOOL", &format!("{function}(a, b)"));
        }
    }
}

/// REQ-KF-analyzer-003: AND, OR and XOR accept every ANY_BIT type and return
/// the operand type.
#[spec_test(REQ_KF_analyzer_003)]
fn analyzer_spec_req_kf_003_bitwise_forms_accept_any_bit() {
    for function in ["AND", "OR", "XOR"] {
        for operand_type in BIT_STRING_TYPES {
            assert_clean(operand_type, operand_type, &format!("{function}(a, b)"));
        }
    }
}

/// REQ-KF-analyzer-004: NOT accepts every ANY_BIT type and returns the operand
/// type. The named-argument spelling is the one that reaches the function form;
/// `NOT(a)` parses as the operator.
#[spec_test(REQ_KF_analyzer_004)]
fn analyzer_spec_req_kf_004_not_form_accepts_any_bit() {
    for operand_type in BIT_STRING_TYPES {
        assert_clean(operand_type, operand_type, "NOT(IN := a)");
    }
}

/// REQ-KF-analyzer-005: an argument outside the operand category is P4026.
#[spec_test(REQ_KF_analyzer_005)]
fn analyzer_spec_req_kf_005_argument_outside_category_is_p4026() {
    let expected = Problem::FunctionCallArgTypeMismatch.code().to_string();
    for (operand_type, expr) in [
        ("INT", "AND(a, b)"),
        ("REAL", "XOR(a, b)"),
        ("INT", "NOT(IN := a)"),
        ("BOOL", "ADD(a, b)"),
        ("STRING", "SUB(a, b)"),
    ] {
        let codes = analyze_codes(&program(operand_type, operand_type, expr));
        assert!(
            codes.contains(&expected),
            "{expr} on {operand_type}: expected {expected}, got {codes:?}"
        );
    }
}
