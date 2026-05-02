//! Bytecode-level integration tests for shift/rotate function compilation.

#[macro_use]
mod common;
use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

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
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 2);

    // x := BYTE#16#0F: LOAD_CONST_I32 pool:0, TRUNC_U8, STORE_VAR_I32 var:0
    // y := SHL(x, 4):  LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1, BUILTIN SHL_I32, TRUNC_U8, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0), // pool:0 (0x0F)
            bc::trunc_u8(),
            bc::dup(),                             // (store-load optimization)
            bc::store_var_i32(0),                  // var:0
            bc::load_const_i32(1),                 // pool:1 (4)
            bc::builtin(opcode::builtin::SHL_I32), // SHL_I32 (0x0348)
            bc::trunc_u8(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
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
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // Verify the ROL on BYTE emits ROL_U8 (0x0350), not ROL_I32
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0), // pool:0 (0x81)
            bc::trunc_u8(),
            bc::dup(),                            // (store-load optimization)
            bc::store_var_i32(0),                 // var:0
            bc::load_const_i32(1),                // pool:1 (1)
            bc::builtin(opcode::builtin::ROL_U8), // ROL_U8 (0x0350)
            bc::trunc_u8(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
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
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // Verify the ROR on WORD emits ROR_U16 (0x0353)
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0), // pool:0 (0x8001)
            bc::trunc_u16(),
            bc::dup(),                             // (store-load optimization)
            bc::store_var_i32(0),                  // var:0
            bc::load_const_i32(1),                 // pool:1 (1)
            bc::builtin(opcode::builtin::ROR_U16), // ROR_U16 (0x0353)
            bc::trunc_u16(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
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
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // DWORD is 32-bit so no TRUNC needed
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0),                 // pool:0 (0x0F)
            bc::dup(),                             // (store-load optimization)
            bc::store_var_i32(0),                  // var:0
            bc::load_const_i32(1),                 // pool:1 (4)
            bc::builtin(opcode::builtin::SHL_I32), // SHL_I32 (0x0348)
            bc::store_var_i32(1),                  // var:1
            bc::ret_void(),
        ]
    );
}
