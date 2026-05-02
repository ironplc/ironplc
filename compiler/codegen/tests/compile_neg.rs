//! Bytecode-level integration tests for the NEG operator compilation.

#[macro_use]
mod common;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

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
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        10
    );

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := -x: LOAD_VAR_I32 var:0, NEG_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
        bc::load_const_i32(0),
        bc::dup(),              // store-load optimization
        bc::store_var_i32(0),
        bc::neg_i32(),
        bc::store_var_i32(1),
        bc::ret_void(),
    ]);
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
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        -5
    );

    // LOAD_CONST_I32 pool:0 (-5), STORE_VAR_I32 var:0, RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
        bc::load_const_i32(0), // (-5)
        bc::store_var_i32(0),
        bc::ret_void(),
    ]);
}
