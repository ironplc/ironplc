//! Bytecode-level integration tests for DUP and NOP peephole optimizations.

use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

#[test]
fn compile_when_same_var_twice_then_emits_dup() {
    // x * x should produce: LOAD_VAR x, DUP, MUL
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := x * x;
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
            bc::load_var_i32(0), // var:0
            bc::dup(),           // (consecutive identical load)
            bc::mul_i32(),
            bc::store_var_i32(1), // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_store_then_load_same_var_then_emits_dup_before_store() {
    // x := 7; y := x; is the classic store-load pattern
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 7;
  y := x;
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
            bc::load_const_i32(0), // pool:0 (7)
            bc::dup(),             // (store-load optimization)
            bc::store_var_i32(0),  // var:0
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_store_load_across_jump_then_no_optimization() {
    // FOR loop: i := 1 TO 5 — the STORE i; LOAD i has a jump target between,
    // so the peephole must NOT apply.
    let source = "
PROGRAM main
  VAR
    i : DINT;
  END_VAR
  FOR i := 1 TO 5 DO
    i := i;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // The first STORE_VAR(i) then LOAD_VAR(i) should NOT be optimized
    // because there is a loop label between them.
    assert_eq!(
        bytecode[3],
        opcode::STORE_VAR_I32,
        "expected STORE_VAR_I32, not DUP"
    );
    assert_eq!(
        bytecode[6],
        opcode::LOAD_VAR_I32,
        "expected LOAD_VAR_I32 after STORE"
    );
}
