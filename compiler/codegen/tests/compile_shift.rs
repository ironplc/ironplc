//! Bytecode-level integration tests for shift/rotate function compilation.

mod common;

use common::parse_and_compile;

#[test]
fn compile_when_shl_byte_then_produces_shl_i32_builtin() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#0F;
  y := SHL(x, 4);
END_PROGRAM
";
    let container = parse_and_compile(source);

    assert_eq!(container.header.num_variables, 2);

    // x := BYTE#16#0F: LOAD_CONST_I32 pool:0, TRUNC_U8, STORE_VAR_I32 var:0
    // y := SHL(x, 4):  LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1, BUILTIN SHL_I32, TRUNC_U8, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0x0F)
            0x21, // TRUNC_U8
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (4)
            0xC4, 0x48, 0x03, // BUILTIN SHL_I32 (0x0348)
            0x21, // TRUNC_U8
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_rol_byte_then_produces_rol_u8_builtin() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#81;
  y := ROL(x, 1);
END_PROGRAM
";
    let container = parse_and_compile(source);

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    // Verify the ROL on BYTE emits ROL_U8 (0x0350), not ROL_I32
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0x81)
            0x21, // TRUNC_U8
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0xC4, 0x50, 0x03, // BUILTIN ROL_U8 (0x0350)
            0x21, // TRUNC_U8
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_ror_word_then_produces_ror_u16_builtin() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
    y : WORD;
  END_VAR
  x := WORD#16#8001;
  y := ROR(x, 1);
END_PROGRAM
";
    let container = parse_and_compile(source);

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    // Verify the ROR on WORD emits ROR_U16 (0x0353)
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0x8001)
            0x23, // TRUNC_U16
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0xC4, 0x53, 0x03, // BUILTIN ROR_U16 (0x0353)
            0x23, // TRUNC_U16
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_shl_dword_then_produces_shl_i32_without_trunc() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#0F;
  y := SHL(x, 4);
END_PROGRAM
";
    let container = parse_and_compile(source);

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    // DWORD is 32-bit so no TRUNC needed
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0x0F)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (4)
            0xC4, 0x48, 0x03, // BUILTIN SHL_I32 (0x0348)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}
