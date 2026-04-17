//! Bytecode-level integration tests for the ABS function with float types.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_abs_real_then_produces_abs_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 5.0;
  y := ABS(x);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := 5.0: LOAD_CONST_F32 pool:0, STORE_VAR_F32 var:0
    // y := ABS(x): LOAD_VAR_F32 var:0, BUILTIN ABS_F32, STORE_VAR_F32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_eq!(
        bytecode,
        &[
            0x03, 0x00, 0x00, // LOAD_CONST_F32 pool:0
            0xA1, // DUP (store-load optimization)
            0x1A, 0x00, 0x00, // STORE_VAR_F32 var:0
            0xC4, 0x54, 0x03, // BUILTIN ABS_F32
            0x1A, 0x01, 0x00, // STORE_VAR_F32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_abs_lreal_then_produces_abs_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 5.0;
  y := ABS(x);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := 5.0: LOAD_CONST_F64 pool:0, STORE_VAR_F64 var:0
    // y := ABS(x): LOAD_VAR_F64 var:0, BUILTIN ABS_F64, STORE_VAR_F64 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_eq!(
        bytecode,
        &[
            0x04, 0x00, 0x00, // LOAD_CONST_F64 pool:0
            0xA1, // DUP (store-load optimization)
            0x1B, 0x00, 0x00, // STORE_VAR_F64 var:0
            0xC4, 0x55, 0x03, // BUILTIN ABS_F64
            0x1B, 0x01, 0x00, // STORE_VAR_F64 var:1
            0xB5, // RET_VOID
        ]
    );
}
