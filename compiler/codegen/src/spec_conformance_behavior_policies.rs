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

/// Every `STRING_TO_<integer>` function on the block and the target it
/// encodes; the bit-string functions share the unsigned target of their
/// width.
const FUNCTIONS: &[(&str, Target)] = &[
    ("UDINT", Target::U32),
    ("DWORD", Target::U32),
    ("DINT", Target::I32),
    ("USINT", Target::U8),
    ("BYTE", Target::U8),
    ("SINT", Target::I8),
    ("UINT", Target::U16),
    ("WORD", Target::U16),
    ("INT", Target::I16),
    ("LINT", Target::I64),
    ("ULINT", Target::U64),
    ("LWORD", Target::U64),
    ("REAL", Target::F32),
    ("LREAL", Target::F64),
];

/// A program converting the STRING `'1'` to `x : <type_name>` with
/// `STRING_TO_<type_name>`.
fn source(type_name: &str) -> String {
    format!(
        "PROGRAM main
VAR
    s : STRING := '1';
    x : {type_name};
END_VAR
    x := STRING_TO_{type_name}(s);
END_PROGRAM"
    )
}

/// Compiles [`source`] for `type_name` under the given policies and returns
/// the program body.
fn program_bytecode(
    type_name: &str,
    non_numeric: StringToNumNonNumeric,
    failure: StringToNumFailure,
) -> Vec<u8> {
    program_bytecode_of(&source(type_name), non_numeric, failure)
}

/// Compiles `source` under the given policies and returns the program body.
fn program_bytecode_of(
    source: &str,
    non_numeric: StringToNumNonNumeric,
    failure: StringToNumFailure,
) -> Vec<u8> {
    let options = CompilerOptions {
        policy_string_to_num_non_numeric: non_numeric,
        policy_string_to_num_failure: failure,
        ..CompilerOptions::default()
    };
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
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

/// The `BUILTIN` func_ids in `bytecode`, in order, walking the instructions
/// by their sizes so an operand byte is never mistaken for an opcode.
fn builtin_func_ids(bytecode: &[u8]) -> Vec<u16> {
    let mut func_ids = Vec::new();
    let mut pc = 0;
    while pc < bytecode.len() {
        let op = bytecode[pc];
        if op == opcode::BUILTIN {
            func_ids.push(u16::from_le_bytes([bytecode[pc + 1], bytecode[pc + 2]]));
        }
        pc += opcode::instruction_size(op);
    }
    func_ids
}

/// REQ-BP-codegen-001: for every function on the block, the builtin is the
/// one the block gives for its target and the selected policies, and only
/// that operand differs between selections.
#[spec_test(REQ_BP_codegen_001)]
fn codegen_spec_req_bp_001_builtin_selected_by_target_and_both_policies() {
    for (type_name, target) in FUNCTIONS {
        let baseline = program_bytecode(
            type_name,
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Trap,
        );
        for non_numeric in StringToNumNonNumeric::ALL {
            for failure in StringToNumFailure::ALL {
                let bytecode = program_bytecode(type_name, *non_numeric, *failure);
                let expected = str_to_num::func_id(*target, *non_numeric, *failure);
                assert_eq!(builtin_func_ids(&bytecode), vec![expected], "{type_name}");

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
                assert_eq!(
                    differing, expected_differing,
                    "{type_name} {non_numeric:?}/{failure:?}"
                );
            }
        }
    }
}

/// REQ-BP-codegen-002: the conversion emits no `TRUNC_*` of its own; the
/// instruction after the builtin is the value's consumer. The comparison
/// consumes the value directly, where an assignment to a sub-32-bit variable
/// would truncate on the store (a no-op on the in-range value the builtin
/// pushes, but not the conversion's doing).
#[spec_test(REQ_BP_codegen_002)]
fn codegen_spec_req_bp_002_no_truncation_after_the_builtin() {
    for (type_name, _) in FUNCTIONS {
        let source = format!(
            "PROGRAM main
VAR
    s : STRING := '1';
    x : {type_name};
    b : BOOL;
END_VAR
    b := STRING_TO_{type_name}(s) = x;
END_PROGRAM"
        );
        let bytecode = program_bytecode_of(
            &source,
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Trap,
        );
        let position = bytecode.iter().position(|b| *b == opcode::BUILTIN).unwrap();
        // BUILTIN carries a two-byte func_id; the right-hand operand load,
        // at the target's width, is what follows the conversion.
        assert_eq!(
            bytecode[position + 3],
            load_opcode(type_name),
            "{type_name}"
        );
    }
}

/// The load opcode of the slot width a `STRING_TO_<type_name>` result is
/// compared at.
fn load_opcode(type_name: &str) -> u8 {
    match type_name {
        "LINT" | "ULINT" | "LWORD" => opcode::LOAD_VAR_I64,
        "REAL" => opcode::LOAD_VAR_F32,
        "LREAL" => opcode::LOAD_VAR_F64,
        _ => opcode::LOAD_VAR_I32,
    }
}
