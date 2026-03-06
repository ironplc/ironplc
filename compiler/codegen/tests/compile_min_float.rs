//! Bytecode-level integration tests for the MIN function with float types.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_min_real_then_produces_min_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  y := MIN(x, 10.0);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    // y := MIN(x, 10.0): LOAD_VAR_F32 var:0, LOAD_CONST_F32 pool:0, BUILTIN MIN_F32, STORE_VAR_F32 var:1
    assert_eq!(
        bytecode,
        &[
            0x12, 0x00, 0x00, // LOAD_VAR_F32 var:0
            0x03, 0x00, 0x00, // LOAD_CONST_F32 pool:0 (10.0)
            0xC4, 0x56, 0x03, // BUILTIN MIN_F32
            0x1A, 0x01, 0x00, // STORE_VAR_F32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_min_lreal_then_produces_min_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  y := MIN(x, 10.0);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x13, 0x00, 0x00, // LOAD_VAR_F64 var:0
            0x04, 0x00, 0x00, // LOAD_CONST_F64 pool:0 (10.0)
            0xC4, 0x57, 0x03, // BUILTIN MIN_F64
            0x1B, 0x01, 0x00, // STORE_VAR_F64 var:1
            0xB5, // RET_VOID
        ]
    );
}
