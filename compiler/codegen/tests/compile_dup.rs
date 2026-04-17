//! Bytecode-level integration tests for DUP and NOP peephole optimizations.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

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
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0xA1, // DUP (consecutive identical load)
            0x32, // MUL_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
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
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (7)
            0xA1, // DUP (store-load optimization)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
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
    assert_eq!(bytecode[3], 0x18, "expected STORE_VAR_I32, not DUP");
    assert_eq!(bytecode[6], 0x10, "expected LOAD_VAR_I32 after STORE");
}
