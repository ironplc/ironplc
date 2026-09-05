//! VM-specific edge case tests for comparison opcodes (EQ_I32, NE_I32, LT_I32, LE_I32, GT_I32, GE_I32).
//!
//! Basic correctness is covered by end_to_end_cmp.rs.
//! These tests cover boundary comparisons with i32::MIN and i32::MAX that cannot
//! be expressed in IEC 61131-3 source, plus a property test that cross-checks
//! all six comparison opcodes against Rust's operators over the full i32 range.

use proptest::prelude::*;

// Boundary anchor: i32::MIN < i32::MAX → 1. Pins signed comparison across the
// extreme operands the random oracle only samples.
#[test]
fn execute_when_lt_i32_min_vs_max_then_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (i32::MAX)
        0x48,              // LT_I32
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(
        crate::common::run_and_read_i32(&bytecode, 1, &[i32::MIN, i32::MAX]),
        1
    );
}

// Property: cross-check all six comparison opcodes against Rust's `<, <=, >, >=,
// ==, !=`, which produce the 1/0 result the VM is expected to push.
proptest! {
    #[test]
    fn execute_when_cmp_i32_any_then_matches_rust(
        a in any::<i32>(),
        b in any::<i32>(),
        op in 0usize..6,
    ) {
        let (opcode, expected): (u8, i32) = match op {
            0 => (0x40, i32::from(a == b)), // EQ_I32
            1 => (0x44, i32::from(a != b)), // NE_I32
            2 => (0x48, i32::from(a < b)),  // LT_I32
            3 => (0x4C, i32::from(a <= b)), // LE_I32
            4 => (0x50, i32::from(a > b)),  // GT_I32
            _ => (0x54, i32::from(a >= b)), // GE_I32
        };
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (a)
            0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (b)
            opcode,            // comparison
            0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a, b]);
        prop_assert_eq!(result, expected);
    }
}
