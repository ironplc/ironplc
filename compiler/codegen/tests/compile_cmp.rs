//! Bytecode-level integration tests for comparison operator compilation.

#[macro_use]
mod common;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

#[test]
fn compile_when_eq_expression_then_produces_eq_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x = 5;
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
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(1))
            .unwrap(),
        5
    );

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := x = 5: LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1, EQ_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_i32(0),  // pool:0
            bc::dup(),  // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1),  // pool:1
            bc::eq_i32(),
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_ne_expression_then_produces_ne_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x <> 5;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_i32(0),  // pool:0
            bc::dup(),  // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1),  // pool:1
            bc::ne_i32(),
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_lt_expression_then_produces_lt_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x < 5;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_i32(0),  // pool:0
            bc::dup(),  // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1),  // pool:1
            bc::lt_i32(),
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_le_expression_then_produces_le_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x <= 5;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_i32(0),  // pool:0
            bc::dup(),  // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1),  // pool:1
            bc::le_i32(),
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_gt_expression_then_produces_gt_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x > 5;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_i32(0),  // pool:0
            bc::dup(),  // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1),  // pool:1
            bc::gt_i32(),
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_ge_expression_then_produces_ge_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x >= 5;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_i32(0),  // pool:0
            bc::dup(),  // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1),  // pool:1
            bc::ge_i32(),
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
    ]);
}
