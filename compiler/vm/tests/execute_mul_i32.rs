//! VM-specific edge case tests for the MUL_I32 opcode.
//!
//! Basic correctness is covered by end_to_end_mul.rs.
//! These tests cover overflow wrapping and stack underflow traps that cannot be expressed in IEC 61131-3 source.

mod common;

use ironplc_container::VarIndex;
use ironplc_vm::error::Trap;

// Overflow: i32::MAX * 2 wraps to -2
#[test]
fn execute_when_mul_i32_max_times_two_then_wraps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MAX)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (2)
        0x32,              // MUL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[i32::MAX, 2]), -2);
}

// Overflow: i32::MIN * 2 wraps to 0
#[test]
fn execute_when_mul_i32_min_times_two_then_wraps_to_zero() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (2)
        0x32,              // MUL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[i32::MIN, 2]), 0);
}

// Overflow: i32::MIN * -1 wraps to i32::MIN (negation of MIN overflows)
#[test]
fn execute_when_mul_i32_min_times_neg_one_then_wraps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (-1)
        0x32,              // MUL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MIN, -1]),
        i32::MIN
    );
}

// Overflow: i32::MAX * i32::MAX wraps
#[test]
fn execute_when_mul_i32_max_times_max_then_wraps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MAX)
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MAX)
        0x32,              // MUL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MAX]),
        i32::MAX.wrapping_mul(i32::MAX)
    );
}

// Overflow: i32::MIN * i32::MIN wraps to 0
#[test]
fn execute_when_mul_i32_min_times_min_then_wraps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MIN)
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MIN)
        0x32,              // MUL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MIN]),
        i32::MIN.wrapping_mul(i32::MIN)
    );
}

// Overflow: i32::MAX * i32::MIN wraps
#[test]
fn execute_when_mul_i32_max_times_min_then_wraps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MAX)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (i32::MIN)
        0x32,              // MUL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MAX, i32::MIN]),
        i32::MAX.wrapping_mul(i32::MIN)
    );
}

#[test]
fn execute_when_mul_i32_stack_underflow_then_trap() {
    assert_eq!(
        common::run_and_expect_trap_i32(&[0x32], 0, &[]),
        Trap::StackUnderflow
    );
}
