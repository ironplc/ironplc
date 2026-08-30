//! Bytecode-level tests for multi-width integer type support.
//!
//! These tests verify that the compiler selects the correct opcodes
//! for different IEC 61131-3 integer types.

use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

#[test]
fn compile_when_sint_then_produces_trunc_i8() {
    // The value is computed at run time. A constant would be truncated at
    // compile time instead, leaving no TRUNC_I8 at all — see
    // compile_const_trunc.rs.
    let source = "
PROGRAM main
  VAR
    x : SINT;
    n : SINT;
  END_VAR
  x := n + 1;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // LOAD_VAR_I32 var:1, LOAD_CONST_I32 pool:0, ADD_I32, TRUNC_I8,
    // STORE_VAR_I32 var:0, RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(1),   // var:1 (n)
            bc::load_const_i32(0), // pool:0 (1)
            bc::add_i32(),
            bc::trunc_i8(),
            bc::store_var_i32(0), // var:0 (x)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_uint_then_produces_trunc_u16() {
    // As above, the value is computed at run time so the TRUNC_U16 survives.
    let source = "
PROGRAM main
  VAR
    x : UINT;
    n : UINT;
  END_VAR
  x := n + 1;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // LOAD_VAR_I32 var:1, LOAD_CONST_I32 pool:0, ADD_I32, TRUNC_U16,
    // STORE_VAR_I32 var:0, RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(1),   // var:1 (n)
            bc::load_const_i32(0), // pool:0 (1)
            bc::add_i32(),
            bc::trunc_u16(),
            bc::store_var_i32(0), // var:0 (x)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_lint_then_produces_i64_opcodes() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := 10;
  y := x + 1;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := 10: LOAD_CONST_I64 pool:0 (10), STORE_VAR_I64 var:0
    // y := x + 1: LOAD_VAR_I64 var:0, LOAD_CONST_I64 pool:1 (1), ADD_I64, STORE_VAR_I64 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i64(0), // pool:0 (10)
            bc::dup(),             // (store-load optimization)
            bc::store_var_i64(0),  // var:0
            bc::load_const_i64(1), // pool:1 (1)
            bc::add_i64(),
            bc::store_var_i64(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_udint_comparison_then_unsigned_opcodes() {
    let source = "
PROGRAM main
  VAR
    x : UDINT;
    y : UDINT;
  END_VAR
  IF x > y THEN
    x := 1;
  END_IF;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // The comparison should use GT_U32 (0x60) instead of GT_I32 (0x50)
    assert!(
        bytecode.contains(&0x60),
        "Expected GT_U32 (0x60) in bytecode: {:02X?}",
        bytecode
    );
}
