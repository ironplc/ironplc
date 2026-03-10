//! VM-specific edge case tests for the BUILTIN ABS_I64 opcode.
//!
//! Basic correctness (positive, negative) is covered by end_to_end_abs_lint.rs.
//! This test covers overflow wrapping that cannot be expressed in IEC 61131-3 source.

mod common;

#[test]
fn execute_when_abs_i64_min_then_wraps() {
    // ABS(i64::MIN) wraps to i64::MIN (wrapping_abs)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (i64::MIN)
        0xC4, 0x61, 0x03,  // BUILTIN ABS_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(
        common::run_and_read_i64(&bytecode, 1, &[i64::MIN]),
        i64::MIN
    );
}
