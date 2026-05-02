//! Bytecode-level integration tests for bitwise operator compilation.

#[macro_use]
mod common;
use ironplc_parser::options::CompilerOptions;

use common::{bc, parse_and_compile};

#[test]
fn compile_when_byte_and_then_produces_bit_and_32_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  y := x AND BYTE#16#0F;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // y := x AND BYTE#16#0F:
    //   LOAD_VAR_I32 var:0
    //   LOAD_CONST_I32 pool:0 (0x0F)
    //   BIT_AND_32 (0x68)
    //   TRUNC_U8 (0x1D)
    //   STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (0x0F)
            bc::bit_and_32(),
            bc::trunc_u8(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_byte_not_then_produces_bit_not_32_with_trunc_u8() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  y := NOT x;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // y := NOT x:
    //   LOAD_VAR_I32 var:0
    //   BIT_NOT_32 (0x74)
    //   TRUNC_U8 (0x1D)  -- inline truncation after NOT
    //   TRUNC_U8 (0x1D)  -- assignment truncation
    //   STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0), // var:0
            bc::bit_not_32(),
            bc::trunc_u8(),       // (inline NOT truncation)
            bc::trunc_u8(),       // (assignment truncation)
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_dint_and_in_comparison_then_still_produces_bool_and() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := x > 0 AND x < 10;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // The AND here is in a comparison context (DINT is signed)
    // so it should still produce BOOL_AND (0x78), not BIT_AND_32.
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (0)
            bc::gt_i32(),
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(1), // pool:1 (10)
            bc::lt_i32(),
            bc::bool_and(),       // (not BIT_AND_32)
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}
