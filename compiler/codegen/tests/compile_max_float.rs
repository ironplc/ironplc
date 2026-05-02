//! Bytecode-level integration tests for the MAX function with float types.

#[macro_use]
mod common;
use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

#[test]
fn compile_when_max_real_then_produces_max_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  y := MAX(x, 10.0);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_f32(0),                   // var:0
            bc::load_const_f32(0),                 // pool:0 (10.0)
            bc::builtin(opcode::builtin::MAX_F32), // MAX_F32
            bc::store_var_f32(1),                  // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_max_lreal_then_produces_max_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  y := MAX(x, 10.0);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_f64(0),                   // var:0
            bc::load_const_f64(0),                 // pool:0 (10.0)
            bc::builtin(opcode::builtin::MAX_F64), // MAX_F64
            bc::store_var_f64(1),                  // var:1
            bc::ret_void(),
        ]
    );
}
