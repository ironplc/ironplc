//! Bytecode-level integration tests for the SUB operator compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_sub_expression_then_produces_sub_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x - 3;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 3);

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := x - 3: LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1, SUB_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1
            0x31, // SUB_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_chain_of_subtractions_then_correct_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 10 - 3 - 2;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // Should have 3 constants: 10, 3, 2
    assert_eq!(container.constant_pool.len(), 3);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 3);
    assert_eq!(container.constant_pool.get_i32(2).unwrap(), 2);

    // (10 - 3) - 2: left-associative evaluation
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (3)
            0x31, // SUB_I32
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (2)
            0x31, // SUB_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
}
