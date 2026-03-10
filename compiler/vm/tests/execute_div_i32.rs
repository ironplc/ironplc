//! VM-specific edge case tests for the DIV_I32 opcode.
//!
//! Basic correctness is covered by end_to_end_div.rs.
//! These tests cover division by zero traps and overflow wrapping that cannot be expressed in IEC 61131-3 source.

mod common;

use ironplc_vm::error::Trap;

#[test]
fn execute_when_div_i32_by_zero_then_trap() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (10)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (0)
        0x33,              // DIV_I32
    ];
    assert_eq!(
        common::run_and_expect_trap_i32(&bytecode, 0, &[10, 0]),
        Trap::DivideByZero
    );
}

#[test]
fn execute_when_div_i32_min_by_neg_one_then_wraps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (-1)
        0x33,              // DIV_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MIN, -1]),
        i32::MIN
    );
}
