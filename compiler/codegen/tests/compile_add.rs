//! Bytecode-level integration tests for the ADD operator compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_add_expression_then_produces_add_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x + 32;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 32);

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := x + 32: LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1, ADD_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1
            0x30, // ADD_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_chain_of_additions_then_correct_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1 + 2 + 3;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // Should have 3 constants: 1, 2, 3
    assert_eq!(container.constant_pool.len(), 3);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 1);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 2);
    assert_eq!(container.constant_pool.get_i32(2).unwrap(), 3);

    // (1 + 2) + 3: left-associative evaluation
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (2)
            0x30, // ADD_I32
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (3)
            0x30, // ADD_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_mixed_add_sub_then_correct_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10 + 5 - 3;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // (10 + 5) - 3
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (5)
            0x30, // ADD_I32
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (3)
            0x31, // SUB_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_mixed_add_mul_then_correct_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 2 + 3 * 4;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // Parser should respect operator precedence: 2 + (3 * 4)
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (2)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (3)
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (4)
            0x32, // MUL_I32
            0x30, // ADD_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
}
