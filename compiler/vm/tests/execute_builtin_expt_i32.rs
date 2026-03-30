//! VM-specific edge case tests for the BUILTIN EXPT_I32 opcode.
//!
//! Basic correctness (e.g., 2**10 = 1024) is covered by end_to_end_expt.rs.
//! These tests cover traps and overflow wrapping that cannot be expressed
//! in IEC 61131-3 source.

mod common;

use ironplc_container::VarIndex;
use ironplc_vm::error::Trap;

#[test]
fn execute_when_expt_i32_negative_exponent_then_trap() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (2)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (-1)
        0xC4, 0x40, 0x03,  // BUILTIN EXPT_I32
    ];
    assert_eq!(
        common::run_and_expect_trap_i32(&bytecode, 0, &[2, -1]),
        Trap::NegativeExponent
    );
}

#[test]
fn execute_when_expt_i32_overflow_then_wraps() {
    // 2 ** 31 wraps to i32::MIN (-2147483648)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (2)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (31)
        0xC4, 0x40, 0x03,  // BUILTIN EXPT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[2, 31]), i32::MIN);
}

#[test]
fn execute_when_invalid_builtin_func_id_then_trap() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)
        0xC4, 0xFF, 0xFF,  // BUILTIN 0xFFFF (unknown)
    ];
    assert_eq!(
        common::run_and_expect_trap_i32(&bytecode, 0, &[1]),
        Trap::InvalidBuiltinFunction(ironplc_container::FunctionId::new(0xFFFF))
    );
}
