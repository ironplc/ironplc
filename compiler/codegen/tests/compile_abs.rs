//! Bytecode-level integration tests for the ABS function compilation.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_abs_function_then_produces_builtin_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := -5;
  y := ABS(x);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(0))
            .unwrap(),
        -5
    );

    // x := -5: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := ABS(x): LOAD_VAR_I32 var:0, BUILTIN ABS_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
            0xA1, // DUP (store-load optimization)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xC4, 0x43, 0x03, // BUILTIN ABS_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}
