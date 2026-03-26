//! Bytecode-level integration tests for the LIMIT function compilation.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_compile;

#[test]
fn compile_when_limit_function_then_produces_builtin_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  y := LIMIT(0, x, 10);
END_PROGRAM
";
    let container = parse_and_compile(source, &ParseOptions::default());

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 5);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 0);
    assert_eq!(container.constant_pool.get_i32(2).unwrap(), 10);

    // x := 5: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := LIMIT(0, x, 10): LOAD_CONST_I32 pool:1, LOAD_VAR_I32 var:0,
    //   LOAD_CONST_I32 pool:2, BUILTIN LIMIT_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (5)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (0)
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (10)
            0xC4, 0x46, 0x03, // BUILTIN LIMIT_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}
