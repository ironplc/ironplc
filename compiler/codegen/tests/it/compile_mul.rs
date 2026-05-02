//! Bytecode-level integration tests for the MUL operator compilation.

use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

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
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        7
    );
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(1))
            .unwrap(),
        6
    );

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0), // pool:0
            bc::dup(),             // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1), // pool:1
            bc::mul_i32(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
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
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Constant folding: (2 * 3) * 4 = 24
    assert_eq!(container.constant_pool.len(), 1);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        24
    );

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0), // pool:0 (24)
            bc::store_var_i32(0),  // var:0
            bc::ret_void(),
        ]
    );
}
