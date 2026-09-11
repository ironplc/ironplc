//! Spec conformance tests for behavior policies (codegen-owned requirements):
//! selecting the `STRING_TO_<numeric>` builtin from the policies.
//!
//! Each test is annotated with `#[spec_test(REQ_BP_codegen_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test
//! fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every codegen-owned requirement has a test.
//!
//! See `specs/design/behavior-policies.md`.

use ironplc_container::builtin::str_to_num::{self, Target};
use ironplc_container::policy::{BehaviorPolicy, StringToNumFailure, StringToNumNonNumeric};
use ironplc_container::{opcode, FunctionId};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

const SOURCE: &str = "PROGRAM main
VAR
    s : STRING := '1';
    x : UDINT;
END_VAR
    x := STRING_TO_UDINT(s);
END_PROGRAM";

/// Compiles [`SOURCE`] under the given policies and returns the program body.
fn program_bytecode(non_numeric: StringToNumNonNumeric, failure: StringToNumFailure) -> Vec<u8> {
    let options = CompilerOptions {
        policy_string_to_num_non_numeric: non_numeric,
        policy_string_to_num_failure: failure,
        ..CompilerOptions::default()
    };
    let library = ironplc_parser::parse_program(SOURCE, &FileId::default(), &options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], &options).unwrap();
    let container = crate::compile(
        &analyzed,
        &ctx,
        &crate::CodegenOptions::from(&options),
        &crate::EmptyLookup,
    )
    .unwrap();
    container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap()
        .to_vec()
}

/// The `BUILTIN` func_ids in `bytecode`, in order.
fn builtin_func_ids(bytecode: &[u8]) -> Vec<u16> {
    bytecode
        .windows(3)
        .filter(|w| w[0] == opcode::BUILTIN)
        .map(|w| u16::from_le_bytes([w[1], w[2]]))
        .collect()
}

/// REQ-BP-codegen-001: the builtin is the one the block gives for the
/// selected policies, and only that operand differs between selections.
#[spec_test(REQ_BP_codegen_001)]
fn codegen_spec_req_bp_001_builtin_selected_by_both_policies() {
    let baseline = program_bytecode(StringToNumNonNumeric::Reject, StringToNumFailure::Trap);
    for non_numeric in StringToNumNonNumeric::ALL {
        for failure in StringToNumFailure::ALL {
            let bytecode = program_bytecode(*non_numeric, *failure);
            let expected = str_to_num::func_id(Target::U32, *non_numeric, *failure);
            assert_eq!(builtin_func_ids(&bytecode), vec![expected]);

            assert_eq!(bytecode.len(), baseline.len());
            let differing = bytecode
                .iter()
                .zip(&baseline)
                .filter(|(a, b)| a != b)
                .count();
            let expected_differing = if expected == builtin_func_ids(&baseline)[0] {
                0
            } else {
                1
            };
            assert_eq!(differing, expected_differing, "{non_numeric:?}/{failure:?}");
        }
    }
}

/// REQ-BP-codegen-002: nothing truncates the conversion's result.
#[spec_test(REQ_BP_codegen_002)]
fn codegen_spec_req_bp_002_no_truncation_after_the_builtin() {
    let bytecode = program_bytecode(StringToNumNonNumeric::Reject, StringToNumFailure::Trap);
    let truncs = [
        opcode::TRUNC_I8,
        opcode::TRUNC_U8,
        opcode::TRUNC_I16,
        opcode::TRUNC_U16,
    ];
    // Every opcode in this program is either an operand-free instruction or
    // one whose operands cannot collide with the TRUNC bytes, so a byte-wise
    // scan is sufficient here: the body is LOAD_CONST_I32, BUILTIN,
    // STORE_VAR_I32, RET_VOID and the string initialisation before it.
    let position = bytecode.iter().position(|b| *b == opcode::BUILTIN).unwrap();
    let after = &bytecode[position + 3..];
    assert!(after.iter().all(|b| !truncs.contains(b)));
}
