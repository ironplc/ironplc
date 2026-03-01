//! Bytecode-level integration tests for boolean operator compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_and_expression_then_produces_bool_and_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x > 0 AND x < 10;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 2);

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := x > 0 AND x < 10:
    //   LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1 (0), GT_I32
    //   LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:0 (10), LT_I32
    //   BOOL_AND
    //   STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (0)
            0x6C, // GT_I32
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x6A, // LT_I32
            0x54, // BOOL_AND
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_or_expression_then_produces_bool_or_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x > 0 OR x < 10;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (0)
            0x6C, // GT_I32
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x6A, // LT_I32
            0x55, // BOOL_OR
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_xor_expression_then_produces_bool_xor_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x > 0 XOR x < 10;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (0)
            0x6C, // GT_I32
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x6A, // LT_I32
            0x56, // BOOL_XOR
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_not_expression_then_produces_bool_not_bytecode() {
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

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := NOT x: LOAD_VAR_I32 var:0, BOOL_NOT, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (10)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x57, // BOOL_NOT
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}
