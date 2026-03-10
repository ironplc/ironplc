//! VM-specific edge case tests for comparison opcodes (EQ_I32, NE_I32, LT_I32, LE_I32, GT_I32, GE_I32).
//!
//! Basic correctness is covered by end_to_end_cmp.rs.
//! These tests cover boundary comparisons with i32::MIN and i32::MAX that cannot be expressed in IEC 61131-3 source.

mod common;

// i32::MIN < i32::MAX → 1
#[test]
fn execute_when_lt_i32_min_vs_max_then_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (i32::MAX)
        0x6A,              // LT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MIN, i32::MAX]),
        1
    );
}

// i32::MAX > i32::MIN → 1
#[test]
fn execute_when_gt_i32_max_vs_min_then_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MAX)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (i32::MIN)
        0x6C,              // GT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MAX, i32::MIN]),
        1
    );
}

// i32::MIN == i32::MIN → 1
#[test]
fn execute_when_eq_i32_min_vs_min_then_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)
        0x68,              // EQ_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[i32::MIN]), 1);
}

// i32::MIN != i32::MAX → 1
#[test]
fn execute_when_ne_i32_min_vs_max_then_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (i32::MAX)
        0x69,              // NE_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MIN, i32::MAX]),
        1
    );
}
