//! Bytecode-level integration tests for MUX function compilation.

#[macro_use]
mod common;
use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

#[test]
fn compile_when_mux_3_inputs_then_produces_builtin_bytecode() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(1, 10, 20, 30);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 1);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        1
    );
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(1))
            .unwrap(),
        10
    );
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(2))
            .unwrap(),
        20
    );
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(3))
            .unwrap(),
        30
    );

    // y := MUX(1, 10, 20, 30):
    //   LOAD_CONST_I32 pool:0 (K=1)
    //   LOAD_CONST_I32 pool:1 (IN0=10)
    //   LOAD_CONST_I32 pool:2 (IN1=20)
    //   LOAD_CONST_I32 pool:3 (IN2=30)
    //   BUILTIN MUX_I32_BASE+3 (0x0403)
    //   STORE_VAR_I32 var:0
    //   RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_i32(0),  // pool:0 (1)
            bc::load_const_i32(1),  // pool:1 (10)
            bc::load_const_i32(2),  // pool:2 (20)
            bc::load_const_i32(3),  // pool:3 (30)
            bc::builtin(opcode::builtin::MUX_I32_BASE + 3),  // MUX with 3 inputs
            bc::store_var_i32(0),  // var:0
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_mux_2_inputs_then_produces_builtin_bytecode() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(0, 100, 200);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // BUILTIN MUX_I32_BASE+2 = 0x0402
    assert_bytecode!(bytecode, [
            bc::load_const_i32(0),  // pool:0 (0)
            bc::load_const_i32(1),  // pool:1 (100)
            bc::load_const_i32(2),  // pool:2 (200)
            bc::builtin(opcode::builtin::MUX_I32_BASE + 2),  // MUX with 2 inputs
            bc::store_var_i32(0),  // var:0
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_mux_with_variable_selector_then_produces_correct_bytecode() {
    let source = "
PROGRAM main
  VAR
    k : DINT;
    y : DINT;
  END_VAR
  k := 2;
  y := MUX(k, 10, 20, 30);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 2);
    // k := 2 uses pool:0
    // MUX uses pool:1(10), pool:2(20), pool:3(30)
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        2
    );
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(1))
            .unwrap(),
        10
    );
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(2))
            .unwrap(),
        20
    );
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(3))
            .unwrap(),
        30
    );
}
