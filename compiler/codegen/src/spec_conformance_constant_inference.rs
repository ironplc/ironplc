//! Spec conformance tests for folding `LEN` of a string that cannot change
//! (codegen-owned requirements of the constant variable inference design).
//!
//! Each test is annotated with `#[spec_test(REQ_CVI_codegen_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test
//! fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every codegen-owned requirement has a test.
//!
//! See `specs/design/constant-variable-inference.md`.

use ironplc_container::{opcode, ConstantIndex, Container, FunctionId};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

fn compile(source: &str) -> Container {
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], &options).unwrap();
    crate::compile(
        &analyzed,
        &ctx,
        &crate::CodegenOptions::from(&options),
        &crate::EmptyLookup,
    )
    .unwrap()
}

/// The opcodes of the scan function, walking the instructions by their
/// sizes so an operand byte is never mistaken for an opcode.
fn scan_opcodes(container: &Container) -> Vec<u8> {
    let bytecode = container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap();
    let mut opcodes = Vec::new();
    let mut pc = 0;
    while pc < bytecode.len() {
        opcodes.push(bytecode[pc]);
        pc += opcode::instruction_size(bytecode[pc]);
    }
    opcodes
}

/// The value of the scan function's `LOAD_CONST_I32`.
fn loaded_i32(container: &Container) -> i32 {
    let bytecode = container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap();
    assert_eq!(bytecode[0], opcode::LOAD_CONST_I32);
    let index = u16::from_le_bytes([bytecode[1], bytecode[2]]);
    container
        .constant_pool
        .get_i32(ConstantIndex::new(index))
        .unwrap()
}

/// A program assigning `LEN(<operand>)` to `n`, after `declarations`.
fn len_program(declarations: &str, operand: &str) -> String {
    format!(
        "PROGRAM main
{declarations}
VAR
    n : DINT;
END_VAR
    n := LEN({operand});
END_PROGRAM"
    )
}

/// REQ-CVI-codegen-060: `LEN` of a literal is one constant load of its
/// length in code units, with no slot or temp buffer for the literal.
#[spec_test(REQ_CVI_codegen_060)]
fn codegen_spec_req_cvi_060_len_of_literal_is_constant_length() {
    let narrow = compile(&len_program("", "'a$'$N$41'"));
    assert_eq!(
        scan_opcodes(&narrow),
        vec![
            opcode::LOAD_CONST_I32,
            opcode::STORE_VAR_I32,
            opcode::RET_VOID
        ]
    );
    assert_eq!(loaded_i32(&narrow), 4);
    assert_eq!(narrow.header.data_region_bytes, 0);
    assert_eq!(narrow.header.num_temp_bufs, 0);

    let wide = compile(&len_program("", "\"$00E9t$\"\""));
    assert_eq!(loaded_i32(&wide), 3);
    assert_eq!(wide.header.data_region_bytes, 0);
    assert_eq!(wide.header.num_temp_bufs, 0);
}

/// REQ-CVI-codegen-061: `LEN(CONCAT(IN1, IN2))` of constant operands is the
/// sum of their lengths.
#[spec_test(REQ_CVI_codegen_061)]
fn codegen_spec_req_cvi_061_len_of_concat_of_constants_is_sum() {
    let container = compile(&len_program(
        "VAR CONSTANT\n    s : STRING[10] := 'abc';\nEND_VAR",
        "CONCAT(CONCAT(s, 'de'), 'f')",
    ));
    assert_eq!(
        scan_opcodes(&container),
        vec![
            opcode::LOAD_CONST_I32,
            opcode::STORE_VAR_I32,
            opcode::RET_VOID
        ]
    );
    assert_eq!(loaded_i32(&container), 6);
}

/// REQ-CVI-codegen-062: `LEN` of a `CONSTANT` string, declared or inferred,
/// is the length of its initial value, capped at its capacity.
#[spec_test(REQ_CVI_codegen_062)]
fn codegen_spec_req_cvi_062_len_of_constant_variable_is_initial_value_length() {
    let declared = compile(&len_program(
        "VAR CONSTANT\n    s : STRING := 'Hello';\nEND_VAR",
        "s",
    ));
    assert_eq!(
        scan_opcodes(&declared),
        vec![
            opcode::LOAD_CONST_I32,
            opcode::STORE_VAR_I32,
            opcode::RET_VOID
        ]
    );
    assert_eq!(loaded_i32(&declared), 5);

    let inferred = compile(&len_program(
        "VAR\n    s : WSTRING := \"héllo\";\nEND_VAR",
        "s",
    ));
    assert_eq!(loaded_i32(&inferred), 5);

    let capped = compile(&len_program(
        "VAR CONSTANT\n    s : STRING[2] := 'Hello';\nEND_VAR",
        "s",
    ));
    assert_eq!(loaded_i32(&capped), 2);
}

/// REQ-CVI-codegen-063: `LEN` of a written string reads it with `LEN_STR`.
#[spec_test(REQ_CVI_codegen_063)]
fn codegen_spec_req_cvi_063_len_of_written_variable_reads_at_run_time() {
    let source = "PROGRAM main
VAR
    s : STRING := 'Hello';
    n : DINT;
END_VAR
    s := 'Hi';
    n := LEN(s);
END_PROGRAM";
    assert!(scan_opcodes(&compile(source)).contains(&opcode::LEN_STR));
}
