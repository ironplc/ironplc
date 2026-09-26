//! Spec conformance tests for arithmetic operator overloads (codegen-owned
//! requirements that compare bytecode).
//!
//! Each test is annotated with `#[spec_test(REQ_AO_codegen_NNN)]`, which
//! adds `#[test]` and references a build-script-generated constant so the
//! test fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance`
//! asserts every codegen-owned requirement has a test.
//! The requirements checked by value run on the VM in
//! `tests/it/end_to_end_arithmetic_overloads.rs`.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_analyzer::{typed_overload, Overload};
use ironplc_container::FunctionId;
use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::Operator;
use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::compile_time_arith::time_arith_for;

/// Compiles `source` under the Edition 3 dialect, which declares the long
/// types, and returns the bytecode of its program body.
fn program_bytecode(source: &str) -> Vec<u8> {
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
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

/// A program assigning `expr`, over `a : left`, `b : right` and
/// `c : right`, to `result : result_type`.
fn program(left: &str, right: &str, result_type: &str, expr: &str) -> String {
    format!(
        "PROGRAM main
VAR
    a : {left};
    b : {right};
    c : {right};
    result : {result_type};
END_VAR
    result := {expr};
END_PROGRAM"
    )
}

/// REQ-AO-codegen-001: an operator expression on a Table 30 pair compiles to
/// the same bytecode as the call to its typed name, at both widths.
#[spec_test(REQ_AO_codegen_001)]
#[rstest]
#[case("TIME", "TIME", "TIME", "a + b", "ADD_TIME(a, b)")]
#[case("TIME_OF_DAY", "TIME", "TIME_OF_DAY", "a + b", "ADD_TOD_TIME(a, b)")]
#[case("DATE_AND_TIME", "TIME", "DATE_AND_TIME", "a + b", "ADD_DT_TIME(a, b)")]
#[case("TIME", "TIME", "TIME", "a - b", "SUB_TIME(a, b)")]
#[case("DATE", "DATE", "TIME", "a - b", "SUB_DATE_DATE(a, b)")]
#[case("TIME_OF_DAY", "TIME", "TIME_OF_DAY", "a - b", "SUB_TOD_TIME(a, b)")]
#[case("TIME_OF_DAY", "TIME_OF_DAY", "TIME", "a - b", "SUB_TOD_TOD(a, b)")]
#[case("DATE_AND_TIME", "TIME", "DATE_AND_TIME", "a - b", "SUB_DT_TIME(a, b)")]
#[case("DATE_AND_TIME", "DATE_AND_TIME", "TIME", "a - b", "SUB_DT_DT(a, b)")]
#[case("TIME", "REAL", "TIME", "a * b", "MUL_TIME(a, b)")]
#[case("TIME", "DINT", "TIME", "a / b", "DIV_TIME(a, b)")]
#[case("LTIME", "LTIME", "LTIME", "a + b", "ADD_LTIME(a, b)")]
#[case(
    "LTIME_OF_DAY",
    "LTIME",
    "LTIME_OF_DAY",
    "a + b",
    "ADD_LTOD_LTIME(a, b)"
)]
#[case(
    "LDATE_AND_TIME",
    "LTIME",
    "LDATE_AND_TIME",
    "a + b",
    "ADD_LDT_LTIME(a, b)"
)]
#[case("LTIME", "LTIME", "LTIME", "a - b", "SUB_LTIME(a, b)")]
#[case("LDATE", "LDATE", "LTIME", "a - b", "SUB_LDATE_LDATE(a, b)")]
#[case(
    "LTIME_OF_DAY",
    "LTIME",
    "LTIME_OF_DAY",
    "a - b",
    "SUB_LTOD_LTIME(a, b)"
)]
#[case(
    "LTIME_OF_DAY",
    "LTIME_OF_DAY",
    "LTIME",
    "a - b",
    "SUB_LTOD_LTOD(a, b)"
)]
#[case(
    "LDATE_AND_TIME",
    "LTIME",
    "LDATE_AND_TIME",
    "a - b",
    "SUB_LDT_LTIME(a, b)"
)]
#[case(
    "LDATE_AND_TIME",
    "LDATE_AND_TIME",
    "LTIME",
    "a - b",
    "SUB_LDT_LDT(a, b)"
)]
#[case("LTIME", "LREAL", "LTIME", "a * b", "MUL_LTIME(a, b)")]
#[case("LTIME", "LINT", "LTIME", "a / b", "DIV_LTIME(a, b)")]
// A short operand of a long form: `t + lt` is ADD_LTIME.
#[case("TIME", "LTIME", "LTIME", "a + b", "ADD_LTIME(a, b)")]
fn codegen_spec_req_ao_001_operator_on_table_30_pair_compiles_as_typed_call(
    #[case] left: &str,
    #[case] right: &str,
    #[case] result_type: &str,
    #[case] operator: &str,
    #[case] call: &str,
) {
    assert_eq!(
        program_bytecode(&program(left, right, result_type, operator)),
        program_bytecode(&program(left, right, result_type, call)),
        "{operator} on {left}, {right} against {call}"
    );
}

/// REQ-AO-codegen-009: an extensible call on a Table 30 pair folds through
/// the typed routine from the left, so `ADD(a, b, c)` compiles to what
/// `a + b + c` compiles to.
#[spec_test(REQ_AO_codegen_009)]
#[rstest]
#[case("TIME", "TIME", "TIME", "ADD(a, b, c)", "a + b + c")]
#[case("TIME_OF_DAY", "TIME", "TIME_OF_DAY", "ADD(a, b, c)", "a + b + c")]
#[case("LTIME", "LTIME", "LTIME", "ADD(a, b, c)", "a + b + c")]
#[case("TIME", "DINT", "TIME", "MUL(a, b, c)", "a * b * c")]
// The long form applies from the step that meets a long operand on.
#[case("TIME", "LTIME", "LTIME", "ADD(a, b, c)", "a + b + c")]
fn codegen_spec_req_ao_009_extensible_call_folds_through_typed_routine(
    #[case] left: &str,
    #[case] right: &str,
    #[case] result_type: &str,
    #[case] call: &str,
    #[case] operator: &str,
) {
    assert_eq!(
        program_bytecode(&program(left, right, result_type, call)),
        program_bytecode(&program(left, right, result_type, operator)),
        "{call} on {left}, {right} against {operator}"
    );
}

/// Every typed name the analyzer's typed step can answer with has a codegen
/// routine, at both widths: the dispatch in `compile_arith` relies on it.
#[test]
fn time_arith_for_when_any_typed_overload_then_has_routine() {
    let types = [
        "TIME",
        "LTIME",
        "DATE",
        "LDATE",
        "TIME_OF_DAY",
        "LTIME_OF_DAY",
        "DATE_AND_TIME",
        "LDATE_AND_TIME",
        "DINT",
        "LINT",
        "REAL",
        "LREAL",
        "ANY_INT",
        "ANY_REAL",
    ];
    let mut names = std::collections::BTreeSet::new();
    for op in [Operator::Add, Operator::Sub, Operator::Mul, Operator::Div] {
        for left in types {
            for right in types {
                let answer = typed_overload(&op, &TypeName::from(left), &TypeName::from(right));
                if let Some(Overload::Typed { name, .. }) = answer {
                    assert!(
                        time_arith_for(&name.to_ascii_lowercase()).is_some(),
                        "{name} has no routine"
                    );
                    names.insert(name);
                }
            }
        }
    }
    // Every overload in both widths is reachable from some pair.
    assert_eq!(names.len(), 22);
}
