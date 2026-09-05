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
fn compile_when_and_then_expression_then_branches_past_right_operand() {
    // AND_THEN must not evaluate its right operand when the left one is
    // FALSE, so it compiles to a branch rather than to BOOL_AND over two
    // eagerly-evaluated values.
    // See specs/design/beckhoff-twincat-dialect.md §3.4.
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x > 0 AND_THEN x < 10;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    };
    let container = parse_and_compile(source, &options);

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
            bc::jmp_if_not(10),    // left FALSE -> LOAD_FALSE, skipping `x < 10`
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (10)
            bc::lt_i32(),
            bc::jmp(1),
            bc::load_false(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_or_else_expression_then_branches_past_right_operand() {
    // OR_ELSE is the dual: a TRUE left operand answers TRUE without
    // evaluating the right operand.
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x > 0 OR_ELSE x < 10;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    };
    let container = parse_and_compile(source, &options);

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
            bc::jmp_if_not(4), // left FALSE -> evaluate `x < 10`
            bc::load_true(),
            bc::jmp(7),
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (10)
            bc::lt_i32(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_and_then_operands_are_bit_strings_then_emits_eager_bitwise_and() {
    // Short-circuiting has no meaning for a bit-string result -- skipping the
    // right operand would produce the left operand's bits rather than their
    // conjunction -- so a non-BOOL AND_THEN degenerates to eager BIT_AND.
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  y := x AND_THEN x;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    };
    let container = parse_and_compile(source, &options);

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0), // var:0
            bc::dup(),           // (consecutive-load optimization)
            bc::bit_and_32(),
            bc::trunc_u8(),       // BYTE is 8-bit storage
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
