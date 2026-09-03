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
fn codegen_spec_req_kf_001_function_form_compiles_as_the_operator() {
    for (operand_type, result_type, call, operator) in [
        ("DINT", "DINT", "ADD(a, b)", "a + b"),
        ("LINT", "LINT", "SUB(a, b)", "a - b"),
        ("REAL", "REAL", "MUL(a, b)", "a * b"),
        ("LREAL", "LREAL", "DIV(a, b)", "a / b"),
        ("INT", "INT", "MOD(a, b)", "a MOD b"),
        ("DINT", "BOOL", "GT(a, b)", "a > b"),
        ("DINT", "BOOL", "GE(a, b)", "a >= b"),
        ("DINT", "BOOL", "EQ(a, b)", "a = b"),
        ("DINT", "BOOL", "LE(a, b)", "a <= b"),
        ("DINT", "BOOL", "LT(a, b)", "a < b"),
        ("DINT", "BOOL", "NE(a, b)", "a <> b"),
        ("BOOL", "BOOL", "AND(a, b)", "a AND b"),
        ("BYTE", "BYTE", "AND(a, b)", "a AND b"),
        ("WORD", "WORD", "OR(a, b)", "a OR b"),
        ("DWORD", "DWORD", "XOR(a, b)", "a XOR b"),
        ("LWORD", "LWORD", "XOR(a, b)", "a XOR b"),
        ("BOOL", "BOOL", "NOT(IN := a)", "NOT a"),
        ("WORD", "WORD", "NOT(IN := a)", "NOT a"),
        ("LWORD", "LWORD", "NOT(IN := a)", "NOT a"),
    ] {
        assert_eq!(
            program_bytecode(&program(operand_type, result_type, call)),
            program_bytecode(&program(operand_type, result_type, operator)),
            "{call} and {operator} on {operand_type} compiled differently"
        );
    }
}
