//! Spec conformance tests for the function forms of operators (codegen-owned
//! requirements).
//!
//! Each test is annotated with `#[spec_test(REQ_KF_codegen_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every codegen-owned requirement has a test.
//!
//! See `specs/design/keyword-function-forms.md`.

use ironplc_container::FunctionId;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;
use spec_test_macro::spec_test;

/// Compiles `source` and returns the bytecode of its program body.
fn program_bytecode(source: &str) -> Vec<u8> {
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], &options).unwrap();
    let container = crate::compile(
        &analyzed,
        &ctx,
        &crate::CodegenOptions::default(),
        &crate::EmptyLookup,
    )
    .unwrap();
    container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap()
        .to_vec()
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

/// REQ-KF-codegen-001: the function form and the operator expression compile
/// to the same bytecode.
#[spec_test(REQ_KF_codegen_001)]
#[rstest]
#[case::add_dint("DINT", "DINT", "ADD(a, b)", "a + b")]
#[case::sub_lint("LINT", "LINT", "SUB(a, b)", "a - b")]
#[case::mul_real("REAL", "REAL", "MUL(a, b)", "a * b")]
#[case::div_lreal("LREAL", "LREAL", "DIV(a, b)", "a / b")]
#[case::mod_int("INT", "INT", "MOD(a, b)", "a MOD b")]
#[case::gt_dint("DINT", "BOOL", "GT(a, b)", "a > b")]
#[case::ge_dint("DINT", "BOOL", "GE(a, b)", "a >= b")]
#[case::eq_dint("DINT", "BOOL", "EQ(a, b)", "a = b")]
#[case::le_dint("DINT", "BOOL", "LE(a, b)", "a <= b")]
#[case::lt_dint("DINT", "BOOL", "LT(a, b)", "a < b")]
#[case::ne_dint("DINT", "BOOL", "NE(a, b)", "a <> b")]
#[case::and_bool("BOOL", "BOOL", "AND(a, b)", "a AND b")]
#[case::and_byte("BYTE", "BYTE", "AND(a, b)", "a AND b")]
#[case::or_word("WORD", "WORD", "OR(a, b)", "a OR b")]
#[case::xor_dword("DWORD", "DWORD", "XOR(a, b)", "a XOR b")]
#[case::xor_lword("LWORD", "LWORD", "XOR(a, b)", "a XOR b")]
#[case::not_bool("BOOL", "BOOL", "NOT(IN := a)", "NOT a")]
#[case::not_word("WORD", "WORD", "NOT(IN := a)", "NOT a")]
#[case::not_lword("LWORD", "LWORD", "NOT(IN := a)", "NOT a")]
fn codegen_spec_req_kf_001_function_form_compiles_as_the_operator(
    #[case] operand_type: &str,
    #[case] result_type: &str,
    #[case] call: &str,
    #[case] operator: &str,
) {
    assert_eq!(
        program_bytecode(&program(operand_type, result_type, call)),
        program_bytecode(&program(operand_type, result_type, operator)),
        "{call} and {operator} on {operand_type} compiled differently"
    );
}

/// A program that assigns `expr`, over operands `a`, `b` and `c` of
/// `operand_type`, to `result` of `operand_type`.
fn program3(operand_type: &str, expr: &str) -> String {
    format!(
        "PROGRAM main
VAR
    a : {operand_type};
    b : {operand_type};
    c : {operand_type};
    result : {operand_type};
END_VAR
    result := {expr};
END_PROGRAM"
    )
}

/// REQ-KF-codegen-002: an n-input call to an extensible form compiles to the
/// operator folded from the left.
#[spec_test(REQ_KF_codegen_002)]
#[rstest]
#[case::add_dint("DINT", "ADD(a, b, c)", "(a + b) + c")]
#[case::add_lreal("LREAL", "ADD(a, b, c)", "(a + b) + c")]
#[case::mul_lint("LINT", "MUL(a, b, c)", "(a * b) * c")]
#[case::mul_real("REAL", "MUL(a, b, c)", "(a * b) * c")]
#[case::and_bool("BOOL", "AND(a, b, c)", "(a AND b) AND c")]
#[case::and_word("WORD", "AND(a, b, c)", "(a AND b) AND c")]
#[case::or_bool("BOOL", "OR(a, b, c)", "(a OR b) OR c")]
#[case::or_lword("LWORD", "OR(a, b, c)", "(a OR b) OR c")]
#[case::xor_bool("BOOL", "XOR(a, b, c)", "(a XOR b) XOR c")]
#[case::xor_dword("DWORD", "XOR(a, b, c)", "(a XOR b) XOR c")]
fn codegen_spec_req_kf_002_extensible_form_compiles_as_the_left_fold(
    #[case] operand_type: &str,
    #[case] call: &str,
    #[case] operator: &str,
) {
    assert_eq!(
        program_bytecode(&program3(operand_type, call)),
        program_bytecode(&program3(operand_type, operator)),
        "{call} and {operator} on {operand_type} compiled differently"
    );
}
