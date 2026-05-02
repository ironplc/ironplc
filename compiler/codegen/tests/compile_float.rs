//! Bytecode-level integration tests for float type compilation.

#[macro_use]
mod common;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

#[test]
fn compile_when_real_then_produces_f32_opcodes() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
  END_VAR
  x := 3.14;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 1);

    // LOAD_CONST_F32 pool:0, STORE_VAR_F32 var:0, RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_f32(0),  // pool:0
            bc::store_var_f32(0),  // var:0
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_lreal_then_produces_f64_opcodes() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.5;
  y := x + 2.5;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 2);

    // x := 1.5: LOAD_CONST_F64 pool:0, STORE_VAR_F64 var:0
    // y := x + 2.5: LOAD_VAR_F64 var:0, LOAD_CONST_F64 pool:1, ADD_F64, STORE_VAR_F64 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(bytecode, [
            bc::load_const_f64(0),  // pool:0
            bc::dup(),  // (store-load optimization)
            bc::store_var_f64(0),  // var:0
            bc::load_const_f64(1),  // pool:1
            bc::add_f64(),
            bc::store_var_f64(1),  // var:1
            bc::ret_void(),
    ]);
}

#[test]
fn compile_when_real_comparison_then_produces_gt_f32() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    result : DINT;
  END_VAR
  x := 5.0;
  y := 3.0;
  IF x > y THEN
    result := 1;
  END_IF;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Verify that the bytecode contains GT_F32 (0x52)
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert!(
        bytecode.contains(&0x52),
        "expected GT_F32 (0x52) in bytecode: {:02X?}",
        bytecode
    );
}
