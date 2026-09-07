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
    #[values("ADD", "SUB", "MUL", "DIV")] function: &str,
    #[values(
        "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "REAL", "LREAL"
    )]
    operand_type: &str,
) {
    assert_clean(operand_type, operand_type, &call(function));
}

/// REQ-KF-analyzer-006: MOD accepts every ANY_INT type and returns the
/// operand type. (A real operand is P4026 by REQ-KF-analyzer-005.)
#[spec_test(REQ_KF_analyzer_006)]
#[rstest]
fn analyzer_spec_req_kf_006_mod_form_accepts_any_int(
    #[values("SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT")] operand_type: &str,
) {
    assert_clean(operand_type, operand_type, &call("MOD"));
}

/// REQ-KF-analyzer-007: the MOD operator is held to the same row as its
/// function form. Every elementary type, the same way as REQ-KF-analyzer-005:
/// a type the row's category admits analyzes clean in both spellings, and any
/// other type is P4049 for the operator where it is P4026 for the form.
#[spec_test(REQ_KF_analyzer_007)]
#[rstest]
fn analyzer_spec_req_kf_007_mod_operator_agrees_with_its_form(
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
    let signature = operator_function_form("MOD").unwrap().signature();
    let category = GenericTypeName::try_from(&signature.parameters[0].param_type.name).unwrap();
    let elementary = ElementaryTypeName::try_from(&Id::from(operand_type)).unwrap();
    let admitted = category.is_compatible_with(&elementary);

    let form_codes = analyze_codes(&program(operand_type, operand_type, "MOD(a, b)"));
    let operator_codes = analyze_codes(&program(operand_type, operand_type, "a MOD b"));
    let p4026 = Problem::FunctionCallArgTypeMismatch.code().to_string();
    let p4049 = Problem::OperatorOperandTypeMismatch.code().to_string();
    if admitted {
        assert!(
            form_codes.is_empty() && operator_codes.is_empty(),
            "MOD on {operand_type}: {category:?} admits it, expected clean analysis, got {form_codes:?} for the form and {operator_codes:?} for the operator"
        );
    } else {
        assert!(
            form_codes.contains(&p4026) && operator_codes.contains(&p4049),
            "MOD on {operand_type}: outside {category:?}, expected {p4026} for the form and {p4049} for the operator, got {form_codes:?} and {operator_codes:?}"
        );
    }
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

/// A program that assigns `expr`, over `n` operands `a1..an` of
/// `operand_type`, to `result` of `result_type`.
fn program_n(operand_type: &str, result_type: &str, n: usize, expr: &str) -> String {
    let decls: String = (1..=n)
        .map(|i| format!("    a{i} : {operand_type};\n"))
        .collect();
    format!(
        "PROGRAM main
VAR
{decls}    result : {result_type};
END_VAR
    result := {expr};
END_PROGRAM"
    )
}

/// The call `function(a1, a2, ..., an)`.
fn call_n(function: &str, n: usize) -> String {
    let args: Vec<String> = (1..=n).map(|i| format!("a{i}")).collect();
    format!("{function}({})", args.join(", "))
}

/// The operand type the extensible forms are exercised on: a numeric type
/// for the arithmetic forms and a bit string for the bitwise ones.
fn extensible_operand_type(function: &str) -> &'static str {
    match function {
        "ADD" | "MUL" => "DINT",
        _ => "WORD",
    }
}

/// REQ-KF-analyzer-008: the extensible forms accept two or more inputs.
#[spec_test(REQ_KF_analyzer_008)]
#[rstest]
fn analyzer_spec_req_kf_008_extensible_forms_accept_two_or_more_inputs(
    #[values("ADD", "MUL", "AND", "OR", "XOR")] function: &str,
    #[values(2, 3, 4, 8)] n: usize,
) {
    let operand_type = extensible_operand_type(function);
    let codes = analyze_codes(&program_n(
        operand_type,
        operand_type,
        n,
        &call_n(function, n),
    ));
    assert!(
        codes.is_empty(),
        "{function} with {n} inputs: expected clean analysis, got {codes:?}"
    );
}

/// REQ-KF-analyzer-009: every other form takes exactly the inputs it
/// declares; one more is P4018.
#[spec_test(REQ_KF_analyzer_009)]
#[rstest]
fn analyzer_spec_req_kf_009_other_forms_reject_a_third_input(
    #[values("SUB", "DIV", "MOD", "GT", "GE", "EQ", "LE", "LT", "NE")] function: &str,
) {
    let codes = analyze_codes(&program_n("DINT", "BOOL", 3, &call_n(function, 3)));
    let p4018 = Problem::FunctionCallWrongArgCount.code().to_string();
    assert!(
        codes.contains(&p4018),
        "{function} with 3 inputs: expected {p4018}, got {codes:?}"
    );
}

/// REQ-KF-analyzer-010: an input beyond the second is checked against the
/// operand category like the first two.
#[spec_test(REQ_KF_analyzer_010)]
#[rstest]
fn analyzer_spec_req_kf_010_third_input_outside_category_is_p4026(
    #[values("ADD", "MUL", "AND", "OR", "XOR")] function: &str,
) {
    let operand_type = extensible_operand_type(function);
    let program = format!(
        "PROGRAM main
VAR
    a : {operand_type};
    s : STRING;
    result : {operand_type};
END_VAR
    result := {function}(a, a, s);
END_PROGRAM"
    );
    let codes = analyze_codes(&program);
    let p4026 = Problem::FunctionCallArgTypeMismatch.code().to_string();
    assert_eq!(
        codes,
        vec![p4026],
        "{function}(a, a, s): expected P4026 for the third input only"
    );
}

/// REQ-KF-analyzer-011: named inputs of an extensible call bind by number.
#[spec_test(REQ_KF_analyzer_011)]
#[rstest]
fn analyzer_spec_req_kf_011_named_inputs_of_extensible_call_bind_by_number(
    #[values("ADD", "MUL", "AND", "OR", "XOR")] function: &str,
) {
    let operand_type = extensible_operand_type(function);
    let expr = format!("{function}(IN3 := a3, IN1 := a1, IN2 := a2)");
    let codes = analyze_codes(&program_n(operand_type, operand_type, 3, &expr));
    assert!(
        codes.is_empty(),
        "{expr}: expected clean analysis, got {codes:?}"
    );
}
