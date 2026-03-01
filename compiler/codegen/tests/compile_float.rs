//! Bytecode-level integration tests for float type compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_real_then_produces_f32_opcodes() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
  END_VAR
  x := 3.14;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 1);

    // LOAD_CONST_F32 pool:0, STORE_VAR_F32 var:0, RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x03, 0x00, 0x00, // LOAD_CONST_F32 pool:0
            0x1A, 0x00, 0x00, // STORE_VAR_F32 var:0
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_lreal_then_produces_f64_opcodes() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.5;
  y := x + 2.5;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 2);

    // x := 1.5: LOAD_CONST_F64 pool:0, STORE_VAR_F64 var:0
    // y := x + 2.5: LOAD_VAR_F64 var:0, LOAD_CONST_F64 pool:1, ADD_F64, STORE_VAR_F64 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x04, 0x00, 0x00, // LOAD_CONST_F64 pool:0
            0x1B, 0x00, 0x00, // STORE_VAR_F64 var:0
            0x13, 0x00, 0x00, // LOAD_VAR_F64 var:0
            0x04, 0x01, 0x00, // LOAD_CONST_F64 pool:1
            0x4E, // ADD_F64
            0x1B, 0x01, 0x00, // STORE_VAR_F64 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_real_comparison_then_produces_gt_f32() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    result : DINT;
  END_VAR
  x := 5.0;
  y := 3.0;
  IF x > y THEN
    result := 1;
  END_IF;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // Verify that the bytecode contains GT_F32 (0x84)
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert!(
        bytecode.contains(&0x84),
        "expected GT_F32 (0x84) in bytecode: {:02X?}",
        bytecode
    );
}
