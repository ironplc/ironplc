//! Bytecode-level integration tests for the NOT operator compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_not_expression_then_produces_not_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := NOT x;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := NOT x: LOAD_VAR_I32 var:0, NOT_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x40, // NOT_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_not_constant_then_produces_not_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := NOT 0;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 0);

    // x := NOT 0: LOAD_CONST_I32 pool:0, NOT_I32, STORE_VAR_I32 var:0
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
            0x40, // NOT_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
}
