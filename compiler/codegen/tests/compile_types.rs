//! Bytecode-level tests for multi-width integer type support.
//!
//! These tests verify that the compiler selects the correct opcodes
//! for different IEC 61131-3 integer types.

mod common;

use common::parse;
use ironplc_codegen::compile;

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
    let library = parse(source);
    let container = compile(&library).unwrap();

    // LOAD_CONST_I32 pool:0, TRUNC_I8, STORE_VAR_I32 var:0, RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (42)
            0x20, // TRUNC_I8
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
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
    let library = parse(source);
    let container = compile(&library).unwrap();

    // LOAD_CONST_I32 pool:0, TRUNC_U16, STORE_VAR_I32 var:0, RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1000)
            0x23, // TRUNC_U16
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
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
    let library = parse(source);
    let container = compile(&library).unwrap();

    // x := 10: LOAD_CONST_I64 pool:0 (10), STORE_VAR_I64 var:0
    // y := x + 1: LOAD_VAR_I64 var:0, LOAD_CONST_I64 pool:1 (1), ADD_I64, STORE_VAR_I64 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x02, 0x00, 0x00, // LOAD_CONST_I64 pool:0 (10)
            0x19, 0x00, 0x00, // STORE_VAR_I64 var:0
            0x11, 0x00, 0x00, // LOAD_VAR_I64 var:0
            0x02, 0x01, 0x00, // LOAD_CONST_I64 pool:1 (1)
            0x38, // ADD_I64
            0x19, 0x01, 0x00, // STORE_VAR_I64 var:1
            0xB5, // RET_VOID
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
    let library = parse(source);
    let container = compile(&library).unwrap();

    let bytecode = container.code.get_function_bytecode(0).unwrap();
    // The comparison should use GT_U32 (0x7A) instead of GT_I32 (0x6C)
    assert!(
        bytecode.contains(&0x7A),
        "Expected GT_U32 (0x7A) in bytecode: {:02X?}",
        bytecode
    );
}
