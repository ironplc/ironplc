//! Bytecode-level integration tests for the NEG operator compilation.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_neg_variable_then_produces_neg_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := -x;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := -x: LOAD_VAR_I32 var:0, NEG_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x35, // NEG_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_neg_literal_then_constant_folds() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := -5;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Constant folding: -5 is stored directly in the pool, no NEG_I32 opcode
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), -5);

    // LOAD_CONST_I32 pool:0 (-5), STORE_VAR_I32 var:0, RET_VOID
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(bytecode, &[0x01, 0x00, 0x00, 0x18, 0x00, 0x00, 0xB5]);
}
