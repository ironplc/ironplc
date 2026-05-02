//! Bytecode-level tests for multi-width integer type support.
//!
//! These tests verify that the compiler selects the correct opcodes
//! for different IEC 61131-3 integer types.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_sint_then_produces_trunc_i8() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
  END_VAR
  x := 42;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // LOAD_CONST_I32 pool:0, TRUNC_I8, STORE_VAR_I32 var:0, RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_eq!(
        bytecode,
        &[
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (42)
            0x1C, // TRUNC_I8
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x8C, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_uint_then_produces_trunc_u16() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
  END_VAR
  x := 1000;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // LOAD_CONST_I32 pool:0, TRUNC_U16, STORE_VAR_I32 var:0, RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_eq!(
        bytecode,
        &[
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1000)
            0x1F, // TRUNC_U16
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x8C, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_lint_then_produces_i64_opcodes() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := 10;
  y := x + 1;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := 10: LOAD_CONST_I64 pool:0 (10), STORE_VAR_I64 var:0
    // y := x + 1: LOAD_VAR_I64 var:0, LOAD_CONST_I64 pool:1 (1), ADD_I64, STORE_VAR_I64 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I64 pool:0 (10)
            0x91, // DUP (store-load optimization)
            0x11, 0x00, 0x00, // STORE_VAR_I64 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I64 pool:1 (1)
            0x21, // ADD_I64
            0x11, 0x01, 0x00, // STORE_VAR_I64 var:1
            0x8C, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_udint_comparison_then_unsigned_opcodes() {
    let source = "
PROGRAM main
  VAR
    x : UDINT;
    y : UDINT;
  END_VAR
  IF x > y THEN
    x := 1;
  END_IF;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // The comparison should use GT_U32 (0x60) instead of GT_I32 (0x50)
    assert!(
        bytecode.contains(&0x60),
        "Expected GT_U32 (0x60) in bytecode: {:02X?}",
        bytecode
    );
}
