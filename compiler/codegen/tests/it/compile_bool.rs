//! Bytecode-level integration tests for boolean operator compilation.

use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

#[test]
fn compile_when_and_expression_then_produces_bool_and_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x > 0 AND x < 10;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(container.header.num_variables, 2);

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := x > 0 AND x < 10:
    //   LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:1 (0), GT_I32
    //   LOAD_VAR_I32 var:0, LOAD_CONST_I32 pool:0 (10), LT_I32
    //   BOOL_AND
    //   STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0), // pool:0 (10)
            bc::dup(),             // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1), // pool:1 (0)
            bc::gt_i32(),
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (10)
            bc::lt_i32(),
            bc::bool_and(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_or_expression_then_produces_bool_or_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x > 0 OR x < 10;
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
            bc::load_const_i32(0), // pool:0 (10)
            bc::dup(),             // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1), // pool:1 (0)
            bc::gt_i32(),
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (10)
            bc::lt_i32(),
            bc::bool_or(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_xor_expression_then_produces_bool_xor_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x > 0 XOR x < 10;
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
            bc::load_const_i32(0), // pool:0 (10)
            bc::dup(),             // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::load_const_i32(1), // pool:1 (0)
            bc::gt_i32(),
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (10)
            bc::lt_i32(),
            bc::bool_xor(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_not_expression_then_produces_bool_not_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := NOT x;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
    // y := NOT x: LOAD_VAR_I32 var:0, BOOL_NOT, STORE_VAR_I32 var:1
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_const_i32(0), // pool:0 (10)
            bc::dup(),             // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::bool_not(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_true_literal_then_produces_load_true() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := TRUE;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // y := TRUE: LOAD_TRUE, STORE_VAR_I32 var:0
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_true(),
            bc::store_var_i32(0), // var:0
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_false_literal_then_produces_load_false() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := FALSE;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // y := FALSE: LOAD_FALSE, STORE_VAR_I32 var:0
    // RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_false(),
            bc::store_var_i32(0), // var:0
            bc::ret_void(),
        ]
    );
}
