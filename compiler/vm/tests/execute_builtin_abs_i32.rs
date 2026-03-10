//! VM-specific edge case tests for the BUILTIN ABS_I32 opcode.
//!
//! Basic correctness (positive, negative, zero) is covered by end_to_end_abs.rs.
//! This test covers overflow wrapping that cannot be expressed in IEC 61131-3 source.

mod common;

#[test]
fn execute_when_abs_i32_min_then_wraps() {
    // ABS(i32::MIN) wraps to i32::MIN (wrapping_abs)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)
        0xC4, 0x43, 0x03,  // BUILTIN ABS_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i32(&bytecode, 1, &[i32::MIN]),
        i32::MIN
    );
}
