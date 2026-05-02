//! Bytecode-level integration tests for the SQRT function compilation.

use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

#[test]
fn compile_when_sqrt_real_then_produces_sqrt_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 9.0;
  y := SQRT(x);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := 9.0: LOAD_CONST_F32 pool:0, STORE_VAR_F32 var:0
    // y := SQRT(x): LOAD_VAR_F32 var:0, BUILTIN SQRT_F32, STORE_VAR_F32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_f32(0),                  // pool:0
            bc::dup(),                              // (store-load optimization)
            bc::store_var_f32(0),                   // var:0
            bc::builtin(opcode::builtin::SQRT_F32), // SQRT_F32
            bc::store_var_f32(1),                   // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_sqrt_lreal_then_produces_sqrt_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 9.0;
  y := SQRT(x);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := 9.0: LOAD_CONST_F64 pool:0, STORE_VAR_F64 var:0
    // y := SQRT(x): LOAD_VAR_F64 var:0, BUILTIN SQRT_F64, STORE_VAR_F64 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_f64(0),                  // pool:0
            bc::dup(),                              // (store-load optimization)
            bc::store_var_f64(0),                   // var:0
            bc::builtin(opcode::builtin::SQRT_F64), // SQRT_F64
            bc::store_var_f64(1),                   // var:1
            bc::ret_void(),
        ]
    );
}
