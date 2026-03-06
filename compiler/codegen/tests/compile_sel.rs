//! Bytecode-level integration tests for the SEL function compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_sel_function_then_produces_builtin_bytecode() {
    let source = "
PROGRAM main
  VAR
    g : DINT;
    y : DINT;
  END_VAR
  g := 1;
  y := SEL(g, 10, 20);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    assert_eq!(container.header.num_variables, 2);
    assert_eq!(container.constant_pool.get_i32(0).unwrap(), 1);
    assert_eq!(container.constant_pool.get_i32(1).unwrap(), 10);
    assert_eq!(container.constant_pool.get_i32(2).unwrap(), 20);

    // g := 1: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := SEL(g, 10, 20): LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1,
    //   LOAD_CONST_I32 pool:2, BUILTIN SEL_I32, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (10)
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (20)
            0xC4, 0x47, 0x03, // BUILTIN SEL_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}
