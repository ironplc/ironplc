//! Bytecode-level integration tests for the POW operator compilation.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_pow_expression_then_produces_builtin_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 2;
  y := x ** 10;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 2);
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

    // x := 2: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := x ** 10: LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1, BUILTIN EXPT_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
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
    x : DINT;
  END_VAR
  x := 2 ** 3 ** 2;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Constant folding: (2 ** 3) ** 2 = 8 ** 2 = 64
    assert_eq!(container.constant_pool.len(), 1);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        64
    );

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (64)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB5, // RET_VOID
        ]
    );
}
