//! Bytecode-level integration tests for the SQRT function compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_sqrt_real_then_produces_sqrt_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 9.0;
  y := SQRT(x);
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();

    // x := 9.0: LOAD_CONST_F32 pool:0, STORE_VAR_F32 var:0
    // y := SQRT(x): LOAD_VAR_F32 var:0, BUILTIN SQRT_F32, STORE_VAR_F32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x03, 0x00, 0x00, // LOAD_CONST_F32 pool:0
            0x1A, 0x00, 0x00, // STORE_VAR_F32 var:0
            0x12, 0x00, 0x00, // LOAD_VAR_F32 var:0
            0xC4, 0x5E, 0x03, // BUILTIN SQRT_F32
            0x1A, 0x01, 0x00, // STORE_VAR_F32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_sqrt_lreal_then_produces_sqrt_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 9.0;
  y := SQRT(x);
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();

    // x := 9.0: LOAD_CONST_F64 pool:0, STORE_VAR_F64 var:0
    // y := SQRT(x): LOAD_VAR_F64 var:0, BUILTIN SQRT_F64, STORE_VAR_F64 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x04, 0x00, 0x00, // LOAD_CONST_F64 pool:0
            0x1B, 0x00, 0x00, // STORE_VAR_F64 var:0
            0x13, 0x00, 0x00, // LOAD_VAR_F64 var:0
            0xC4, 0x5F, 0x03, // BUILTIN SQRT_F64
            0x1B, 0x01, 0x00, // STORE_VAR_F64 var:1
            0xB5, // RET_VOID
        ]
    );
}
