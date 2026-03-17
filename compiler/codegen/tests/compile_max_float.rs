//! Bytecode-level integration tests for the MAX function with float types.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_max_real_then_produces_max_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  y := MAX(x, 10.0);
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(
        &library,
        context.functions(),
        context.types(),
        context.reachable(),
    )
    .unwrap();

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x12, 0x00, 0x00, // LOAD_VAR_F32 var:0
            0x03, 0x00, 0x00, // LOAD_CONST_F32 pool:0 (10.0)
            0xC4, 0x58, 0x03, // BUILTIN MAX_F32
            0x1A, 0x01, 0x00, // STORE_VAR_F32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_max_lreal_then_produces_max_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  y := MAX(x, 10.0);
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(
        &library,
        context.functions(),
        context.types(),
        context.reachable(),
    )
    .unwrap();

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x13, 0x00, 0x00, // LOAD_VAR_F64 var:0
            0x04, 0x00, 0x00, // LOAD_CONST_F64 pool:0 (10.0)
            0xC4, 0x59, 0x03, // BUILTIN MAX_F64
            0x1B, 0x01, 0x00, // STORE_VAR_F64 var:1
            0xB5, // RET_VOID
        ]
    );
}
