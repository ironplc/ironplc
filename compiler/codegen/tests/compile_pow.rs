//! Bytecode-level integration tests for the POW operator compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_pow_expression_then_produces_builtin_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 2;
  y := x ** 10;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 2);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 10);

    // x := 2: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := x ** 10: LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1, BUILTIN EXPT_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1
            0xC4, 0x40, 0x03, // BUILTIN EXPT_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_chain_of_pows_then_correct_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 2 ** 3 ** 2;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // Should have 3 constants: 2, 3, 2 (but 2 is deduplicated)
    assert_eq!(container.constant_pool.len(), 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 2);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 3);

    // (2 ** 3) ** 2: left-associative evaluation
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (2)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (3)
            0xC4, 0x40, 0x03, // BUILTIN EXPT_I32
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (2)
            0xC4, 0x40, 0x03, // BUILTIN EXPT_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
}
