//! Bytecode-level tests for folding `TRUNC_*` that follows a constant load —
//! structure only. Behaviour is covered by `end_to_end_const_trunc.rs`.
//!
//! `emit_truncation` emits a `TRUNC_*` before every store to a sub-32-bit
//! slot (ADR-0001). When the value came from a constant load the compiler can
//! settle the truncation itself, so these pin that no `TRUNC_*` reaches the
//! bytecode — and that one still does when the value is computed at run time.

use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

/// Bytecode of the scan function (`FunctionId(1)`).
fn scan_bytecode(source: &str) -> Vec<u8> {
    let container = parse_and_compile(source, &CompilerOptions::default());
    container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap()
        .to_vec()
}

/// Bytecode of the init function (`FunctionId(0)`), where variable
/// initializers and structure field defaults are emitted.
fn init_bytecode(source: &str) -> Vec<u8> {
    let container = parse_and_compile(source, &CompilerOptions::default());
    container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(0))
        .unwrap()
        .to_vec()
}

fn contains_any_trunc(bytecode: &[u8]) -> bool {
    bytecode.iter().any(|&b| {
        b == opcode::TRUNC_I8
            || b == opcode::TRUNC_U8
            || b == opcode::TRUNC_I16
            || b == opcode::TRUNC_U16
    })
}

#[test]
fn compile_when_sint_constant_store_then_no_trunc() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : SINT;
  END_VAR
  x := 42;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_const_i32(0), // pool:0 (42)
            bc::store_var_i32(0),  // var:0
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_uint_constant_store_then_no_trunc() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : UINT;
  END_VAR
  x := 1000;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_const_i32(0), // pool:0 (1000)
            bc::store_var_i32(0),  // var:0
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_constant_out_of_range_then_folded_not_truncated() {
    // 300 does not fit USINT. The truncation still happens — it happens at
    // compile time, so the pool holds 44 and no TRUNC_U8 is emitted.
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : USINT;
  END_VAR
  x := 300;
END_PROGRAM
",
    );

    assert!(
        !contains_any_trunc(&bytecode),
        "out-of-range constant should be truncated at compile time; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_narrow_array_element_constant_store_then_no_trunc() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF SINT;
  END_VAR
  arr[1] := 42;
END_PROGRAM
",
    );

    assert!(
        !contains_any_trunc(&bytecode),
        "constant array-element store should not truncate at run time; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_struct_narrow_field_init_then_no_trunc() {
    // Structure field initialization emits a constant load per field, so this
    // is where the fold does the most work.
    let bytecode = init_bytecode(
        "
TYPE
  Motor : STRUCT
    speed : INT;
    status : BYTE;
    fault : SINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    m : Motor := (speed := 100, status := BYTE#16#0F, fault := -3);
  END_VAR
  m.speed := 33;
END_PROGRAM
",
    );

    assert!(
        !contains_any_trunc(&bytecode),
        "structure field initializers are constants and should not truncate at \
         run time; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_value_is_computed_then_trunc_remains() {
    // The counterpart to the tests above: nothing is folded away when the
    // value is not known until the scan runs.
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : INT;
    n : INT;
  END_VAR
  x := n + 1;
END_PROGRAM
",
    );

    assert!(
        bytecode.contains(&opcode::TRUNC_I16),
        "a run-time value must still be truncated; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_dup_precedes_trunc_then_trunc_remains() {
    // The emitter's consecutive-load peephole replaces the second load with a
    // DUP, which the fold does not match — the TRUNC has to stay because the
    // pass cannot see the value behind a DUP.
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : SINT;
    n : SINT;
  END_VAR
  x := n + n;
END_PROGRAM
",
    );

    assert!(
        bytecode.contains(&opcode::DUP),
        "expected the consecutive-load peephole to fire; bytecode = {bytecode:?}"
    );
    assert!(
        bytecode.contains(&opcode::TRUNC_I8),
        "TRUNC after a computed value must remain; bytecode = {bytecode:?}"
    );
}
