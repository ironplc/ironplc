//! Bytecode-level integration tests for the SEL function with float types.

#[macro_use]
mod common;
use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

#[test]
fn compile_when_sel_real_then_produces_sel_f32_bytecode() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := SEL(0, 10.0, 20.0);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // y := SEL(0, 10.0, 20.0):
    //   LOAD_CONST_I32 pool:0 (0)    -- G is always i32
    //   LOAD_CONST_F32 pool:1 (10.0) -- IN0
    //   LOAD_CONST_F32 pool:2 (20.0) -- IN1
    //   BUILTIN SEL_F32
    //   STORE_VAR_F32 var:0
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0),                 // pool:0 (0)
            bc::load_const_f32(1),                 // pool:1 (10.0)
            bc::load_const_f32(2),                 // pool:2 (20.0)
            bc::builtin(opcode::builtin::SEL_F32), // SEL_F32
            bc::store_var_f32(0),                  // var:0
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_sel_lreal_then_produces_sel_f64_bytecode() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := SEL(1, 10.0, 20.0);
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
            bc::load_const_i32(0),                 // pool:0 (1)
            bc::load_const_f64(1),                 // pool:1 (10.0)
            bc::load_const_f64(2),                 // pool:2 (20.0)
            bc::builtin(opcode::builtin::SEL_F64), // SEL_F64
            bc::store_var_f64(0),                  // var:0
            bc::ret_void(),
        ]
    );
}
