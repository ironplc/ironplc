//! Bytecode-level integration tests for the LIMIT function with float types.

#[macro_use]
mod common;
use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

#[test]
fn compile_when_limit_real_then_produces_limit_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 5.0;
  y := LIMIT(0.0, x, 10.0);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := 5.0: LOAD_CONST_F32 pool:0, STORE_VAR_F32 var:0
    // y := LIMIT(0.0, x, 10.0): LOAD_CONST_F32 pool:1, LOAD_VAR_F32 var:0,
    //   LOAD_CONST_F32 pool:2, BUILTIN LIMIT_F32, STORE_VAR_F32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_f32(0),                   // pool:0 (5.0)
            bc::store_var_f32(0),                    // var:0
            bc::load_const_f32(1),                   // pool:1 (0.0)
            bc::load_var_f32(0),                     // var:0
            bc::load_const_f32(2),                   // pool:2 (10.0)
            bc::builtin(opcode::builtin::LIMIT_F32), // LIMIT_F32
            bc::store_var_f32(1),                    // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_limit_lreal_then_produces_limit_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 5.0;
  y := LIMIT(0.0, x, 10.0);
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
            bc::load_const_f64(0),                   // pool:0 (5.0)
            bc::store_var_f64(0),                    // var:0
            bc::load_const_f64(1),                   // pool:1 (0.0)
            bc::load_var_f64(0),                     // var:0
            bc::load_const_f64(2),                   // pool:2 (10.0)
            bc::builtin(opcode::builtin::LIMIT_F64), // LIMIT_F64
            bc::store_var_f64(1),                    // var:1
            bc::ret_void(),
        ]
    );
}
