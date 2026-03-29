//! Bytecode-level integration tests for MUX function compilation.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

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
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (10)
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (20)
            0x01, 0x03, 0x00, // LOAD_CONST_I32 pool:3 (30)
            0xC4, 0x03, 0x04, // BUILTIN MUX_I32(3) = 0x0403
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
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
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (100)
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (200)
            0xC4, 0x02, 0x04, // BUILTIN MUX_I32(2) = 0x0402
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
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
