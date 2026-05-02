//! Bytecode-level integration tests for the MIN function with float types.

use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

#[test]
fn compile_when_min_real_then_produces_min_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  y := MIN(x, 10.0);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // y := MIN(x, 10.0): LOAD_VAR_F32 var:0, LOAD_CONST_F32 pool:0, BUILTIN MIN_F32, STORE_VAR_F32 var:1
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_f32(0),                   // var:0
            bc::load_const_f32(0),                 // pool:0 (10.0)
            bc::builtin(opcode::builtin::MIN_F32), // MIN_F32
            bc::store_var_f32(1),                  // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_min_lreal_then_produces_min_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  y := MIN(x, 10.0);
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
            bc::builtin(opcode::builtin::MIN_F64), // MIN_F64
            bc::store_var_f64(1),                  // var:1
            bc::ret_void(),
        ]
    );
}
