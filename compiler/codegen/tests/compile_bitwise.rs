//! Bytecode-level integration tests for bitwise operator compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_byte_and_then_produces_bit_and_32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  y := x AND BYTE#16#0F;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // y := x AND BYTE#16#0F:
    //   LOAD_VAR_I32 var:0
    //   LOAD_CONST_I32 pool:0 (0x0F)
    //   BIT_AND_32 (0x58)
    //   TRUNC_U8 (0x21)
    //   STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0x0F)
            0x58, // BIT_AND_32
            0x21, // TRUNC_U8
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_byte_not_then_produces_bit_not_32_with_trunc_u8() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  y := NOT x;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // y := NOT x:
    //   LOAD_VAR_I32 var:0
    //   BIT_NOT_32 (0x5B)
    //   TRUNC_U8 (0x21)  -- inline truncation after NOT
    //   TRUNC_U8 (0x21)  -- assignment truncation
    //   STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x5B, // BIT_NOT_32
            0x21, // TRUNC_U8 (inline NOT truncation)
            0x21, // TRUNC_U8 (assignment truncation)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_dint_and_in_comparison_then_still_produces_bool_and() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := x > 0 AND x < 10;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // The AND here is in a comparison context (DINT is signed)
    // so it should still produce BOOL_AND (0x54), not BIT_AND_32.
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
            0x6C, // GT_I32
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (10)
            0x6A, // LT_I32
            0x54, // BOOL_AND (not BIT_AND_32)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}
