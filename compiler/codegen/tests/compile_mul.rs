//! Bytecode-level integration tests for the MUL operator compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_mul_expression_then_produces_mul_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 7;
  y := x * 6;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 7);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 6);

    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1
            0x32, // MUL_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_chain_of_multiplications_then_correct_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 2 * 3 * 4;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.constant_pool.len(), 3);

    // (2 * 3) * 4: left-associative evaluation
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (2)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (3)
            0x32, // MUL_I32
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (4)
            0x32, // MUL_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
}
