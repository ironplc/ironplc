//! Bytecode-level integration tests for the SEL function with float types.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_sel_real_then_produces_sel_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := SEL(0, 10.0, 20.0);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // y := SEL(0, 10.0, 20.0):
    //   LOAD_CONST_I32 pool:0 (0)    -- G is always i32
    //   LOAD_CONST_F32 pool:1 (10.0) -- IN0
    //   LOAD_CONST_F32 pool:2 (20.0) -- IN1
    //   BUILTIN SEL_F32
    //   STORE_VAR_F32 var:0
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
            0x03, 0x01, 0x00, // LOAD_CONST_F32 pool:1 (10.0)
            0x03, 0x02, 0x00, // LOAD_CONST_F32 pool:2 (20.0)
            0xC4, 0x5C, 0x03, // BUILTIN SEL_F32
            0x1A, 0x00, 0x00, // STORE_VAR_F32 var:0
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_sel_lreal_then_produces_sel_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := SEL(1, 10.0, 20.0);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x04, 0x01, 0x00, // LOAD_CONST_F64 pool:1 (10.0)
            0x04, 0x02, 0x00, // LOAD_CONST_F64 pool:2 (20.0)
            0xC4, 0x5D, 0x03, // BUILTIN SEL_F64
            0x1B, 0x00, 0x00, // STORE_VAR_F64 var:0
            0xB5, // RET_VOID
        ]
    );
}
