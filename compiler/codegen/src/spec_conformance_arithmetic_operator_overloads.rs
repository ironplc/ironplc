//! Spec conformance tests for the arithmetic operator overloads
//! (codegen-owned bytecode requirements).
//!
//! These cover `REQ-AO-codegen-001` and `-009`. They are plain tests named
//! for their requirement until the design is listed in this crate's
//! `build.rs`, which happens once every codegen-owned requirement has a
//! test; the attribute then becomes `#[spec_test(REQ_AO_codegen_NNN)]`.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_container::FunctionId;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

/// Compiles `source` under `options` and returns the bytecode of its
/// program body.
fn program_bytecode(source: &str, options: &CompilerOptions) -> Vec<u8> {
    let library = ironplc_parser::parse_program(source, &FileId::default(), options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], options).unwrap();
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

/// A program that assigns `expr`, over `a` of `left_type` and `b` of
/// `right_type`, to `result` of `result_type`.
fn program(left_type: &str, right_type: &str, result_type: &str, expr: &str) -> String {
    format!(
        "PROGRAM main
VAR
    a : {left_type};
    b : {right_type};
    result : {result_type};
END_VAR
    result := {expr};
END_PROGRAM"
    )
}

/// The eleven Table 30 rows at both widths: the row's operand types, its
/// result type, and the spelling of its operator and typed function.
#[rstest]
#[case::add_time("TIME", "TIME", "TIME", "a + b", "ADD_TIME(a, b)")]
#[case::add_tod_time("TIME_OF_DAY", "TIME", "TIME_OF_DAY", "a + b", "ADD_TOD_TIME(a, b)")]
#[case::add_dt_time("DATE_AND_TIME", "TIME", "DATE_AND_TIME", "a + b", "ADD_DT_TIME(a, b)")]
#[case::sub_time("TIME", "TIME", "TIME", "a - b", "SUB_TIME(a, b)")]
#[case::sub_date_date("DATE", "DATE", "TIME", "a - b", "SUB_DATE_DATE(a, b)")]
#[case::sub_tod_time("TIME_OF_DAY", "TIME", "TIME_OF_DAY", "a - b", "SUB_TOD_TIME(a, b)")]
#[case::sub_tod_tod("TIME_OF_DAY", "TIME_OF_DAY", "TIME", "a - b", "SUB_TOD_TOD(a, b)")]
#[case::sub_dt_time("DATE_AND_TIME", "TIME", "DATE_AND_TIME", "a - b", "SUB_DT_TIME(a, b)")]
#[case::sub_dt_dt("DATE_AND_TIME", "DATE_AND_TIME", "TIME", "a - b", "SUB_DT_DT(a, b)")]
#[case::mul_time("TIME", "DINT", "TIME", "a * b", "MUL_TIME(a, b)")]
#[case::mul_time_real("TIME", "REAL", "TIME", "a * b", "MUL_TIME(a, b)")]
#[case::div_time("TIME", "DINT", "TIME", "a / b", "DIV_TIME(a, b)")]
#[case::div_time_lreal("TIME", "LREAL", "TIME", "a / b", "DIV_TIME(a, b)")]
#[case::add_ltime("LTIME", "LTIME", "LTIME", "a + b", "ADD_LTIME(a, b)")]
#[case::add_ltod_ltime(
    "LTIME_OF_DAY",
    "LTIME",
    "LTIME_OF_DAY",
    "a + b",
    "ADD_LTOD_LTIME(a, b)"
)]
#[case::add_ldt_ltime(
    "LDATE_AND_TIME",
    "LTIME",
    "LDATE_AND_TIME",
    "a + b",
    "ADD_LDT_LTIME(a, b)"
)]
#[case::sub_ltime("LTIME", "LTIME", "LTIME", "a - b", "SUB_LTIME(a, b)")]
#[case::sub_ldate_ldate("LDATE", "LDATE", "LTIME", "a - b", "SUB_LDATE_LDATE(a, b)")]
#[case::sub_ltod_ltime(
    "LTIME_OF_DAY",
    "LTIME",
    "LTIME_OF_DAY",
    "a - b",
    "SUB_LTOD_LTIME(a, b)"
)]
#[case::sub_ltod_ltod(
    "LTIME_OF_DAY",
    "LTIME_OF_DAY",
    "LTIME",
    "a - b",
    "SUB_LTOD_LTOD(a, b)"
)]
#[case::sub_ldt_ltime(
    "LDATE_AND_TIME",
    "LTIME",
    "LDATE_AND_TIME",
    "a - b",
    "SUB_LDT_LTIME(a, b)"
)]
#[case::sub_ldt_ldt(
    "LDATE_AND_TIME",
    "LDATE_AND_TIME",
    "LTIME",
    "a - b",
    "SUB_LDT_LDT(a, b)"
)]
#[case::mul_ltime("LTIME", "DINT", "LTIME", "a * b", "MUL_LTIME(a, b)")]
#[case::div_ltime("LTIME", "REAL", "LTIME", "a / b", "DIV_LTIME(a, b)")]
// A pair mixing the widths of one family is the long form.
#[case::add_time_ltime("TIME", "LTIME", "LTIME", "a + b", "ADD_LTIME(a, b)")]
#[case::sub_ldt_dt(
    "LDATE_AND_TIME",
    "DATE_AND_TIME",
    "LTIME",
    "a - b",
    "SUB_LDT_LDT(a, b)"
)]
/// REQ-AO-codegen-001: an operator expression on a Table 30 pair compiles
/// to the same bytecode as the call to its typed name.
fn codegen_spec_req_ao_001_operator_on_table_30_pair_compiles_as_typed_call(
    #[case] left_type: &str,
    #[case] right_type: &str,
    #[case] result_type: &str,
    #[case] operator: &str,
    #[case] call: &str,
) {
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    assert_eq!(
        program_bytecode(
            &program(left_type, right_type, result_type, operator),
            &options
        ),
        program_bytecode(&program(left_type, right_type, result_type, call), &options),
        "{operator} and {call} on {left_type}, {right_type} compiled differently"
    );
}

/// REQ-AO-codegen-001, function form: `ADD(a, b)` on a Table 30 pair
/// compiles as the typed call too.
#[rstest]
#[case::add("TIME", "TIME", "TIME", "ADD(a, b)", "ADD_TIME(a, b)")]
#[case::sub("DATE", "DATE", "TIME", "SUB(a, b)", "SUB_DATE_DATE(a, b)")]
#[case::mul("TIME", "REAL", "TIME", "MUL(a, b)", "MUL_TIME(a, b)")]
#[case::div("TIME", "DINT", "TIME", "DIV(a, b)", "DIV_TIME(a, b)")]
#[case::add_long("LTIME", "TIME", "LTIME", "ADD(a, b)", "ADD_LTIME(a, b)")]
fn codegen_spec_req_ao_001_function_form_on_table_30_pair_compiles_as_typed_call(
    #[case] left_type: &str,
    #[case] right_type: &str,
    #[case] result_type: &str,
    #[case] form: &str,
    #[case] call: &str,
) {
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    assert_eq!(
        program_bytecode(&program(left_type, right_type, result_type, form), &options),
        program_bytecode(&program(left_type, right_type, result_type, call), &options),
        "{form} and {call} on {left_type}, {right_type} compiled differently"
    );
}

/// A program that assigns `expr`, over three `TIME` operands, to `result`.
fn program3(expr: &str) -> String {
    format!(
        "PROGRAM main
VAR
    a : TIME;
    b : TIME;
    c : TIME;
    result : TIME;
END_VAR
    result := {expr};
END_PROGRAM"
    )
}

/// REQ-AO-codegen-009: an extensible call on a Table 30 pair compiles to
/// the typed routine folded from the left.
#[rstest]
#[case::add("ADD(a, b, c)", "a + b + c")]
#[case::add_parenthesized("ADD(a, b, c)", "(a + b) + c")]
#[case::add_nested_typed("ADD(a, b, c)", "ADD_TIME(ADD_TIME(a, b), c)")]
#[case::mul("MUL(a, 2, 3)", "a * 2 * 3")]
fn codegen_spec_req_ao_009_extensible_call_on_table_30_pair_folds_typed_routine(
    #[case] call: &str,
    #[case] operator: &str,
) {
    let options = CompilerOptions::default();
    assert_eq!(
        program_bytecode(&program3(call), &options),
        program_bytecode(&program3(operator), &options),
        "{call} and {operator} compiled differently"
    );
}
