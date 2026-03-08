//! Bytecode-level integration tests for function forms of operators.
//!
//! Each test verifies that calling a standard library function form (e.g., ADD(x, 5))
//! produces the same opcode as the equivalent operator (e.g., x + 5).
//!
//! Note: MOD, AND, OR, XOR, NOT function forms are not tested because the parser
//! treats these names as operator keywords. The codegen routing exists but requires
//! parser changes to be reachable.

mod common;

use common::parse;
use ironplc_codegen::compile;

/// Helper to build an IEC 61131-3 program that calls a two-arg function form.
fn two_arg_program(func_name: &str, var_type: &str) -> String {
    format!(
        "
PROGRAM main
  VAR
    x : {var_type};
    y : {var_type};
  END_VAR
  x := 10;
  y := {func_name}(x, 5);
END_PROGRAM
"
    )
}

/// Helper to assert bytecode for a two-arg function form.
/// The expected_opcode is the single-byte opcode that the function should emit.
fn assert_two_arg_bytecode(source: &str, expected_opcode: u8) {
    let library = parse(source);
    let container = compile(&library).unwrap();

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (5)
            expected_opcode,  // The operator opcode
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5,             // RET_VOID
        ]
    );
}

// --- Arithmetic functions ---

#[test]
fn compile_when_add_function_then_produces_add_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("ADD", "DINT"), 0x30);
}

#[test]
fn compile_when_sub_function_then_produces_sub_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("SUB", "DINT"), 0x31);
}

#[test]
fn compile_when_mul_function_then_produces_mul_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("MUL", "DINT"), 0x32);
}

#[test]
fn compile_when_div_function_then_produces_div_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("DIV", "DINT"), 0x33);
}

// --- Comparison functions ---

#[test]
fn compile_when_eq_function_then_produces_eq_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("EQ", "DINT"), 0x68);
}

#[test]
fn compile_when_ne_function_then_produces_ne_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("NE", "DINT"), 0x69);
}

#[test]
fn compile_when_lt_function_then_produces_lt_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("LT", "DINT"), 0x6A);
}

#[test]
fn compile_when_le_function_then_produces_le_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("LE", "DINT"), 0x6B);
}

#[test]
fn compile_when_gt_function_then_produces_gt_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("GT", "DINT"), 0x6C);
}

#[test]
fn compile_when_ge_function_then_produces_ge_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("GE", "DINT"), 0x6D);
}
