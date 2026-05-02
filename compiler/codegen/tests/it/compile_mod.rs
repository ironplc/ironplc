//! Bytecode-level integration tests for the MOD operator compilation.

use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

#[test]
fn compile_when_mod_expression_then_produces_mod_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x MOD 3;
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
        3
    );

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := x MOD 3: LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1, MOD_I32, STORE_VAR_I32 var:1
    // RET_VOID
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
            bc::mod_i32(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_chain_of_modulos_then_correct_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 100 MOD 7 MOD 3;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Constant folding: (100 MOD 7) MOD 3 = 2 MOD 3 = 2
    assert_eq!(container.constant_pool.len(), 1);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        2
    );

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0), // pool:0 (2)
            bc::store_var_i32(0),  // var:0
            bc::ret_void(),
        ]
    );
}
