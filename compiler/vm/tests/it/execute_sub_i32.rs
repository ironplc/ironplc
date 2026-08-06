//! VM-specific edge case tests for the SUB_I32 opcode.
//!
//! Basic subtraction correctness is covered by end_to_end_sub.rs.
//! These tests cover overflow wrapping and trap behavior that cannot be
//! expressed in IEC 61131-3 source, plus a property test that cross-checks the
//! opcode against Rust `wrapping_sub` over the full i32 range.

use ironplc_vm::error::Trap;
use proptest::prelude::*;

// LOAD_CONST_I32 pool[0], LOAD_CONST_I32 pool[1], SUB_I32, STORE_VAR_I32 var[0], RET_VOID
#[rustfmt::skip]
const SUB_BYTECODE: [u8; 11] = [
    0x00, 0x00, 0x00,
    0x00, 0x01, 0x00,
    0x24,
    0x10, 0x00, 0x00,
    0x8C,
];

// Nominal anchor: a straightforward difference with no overflow.
#[test]
fn execute_when_sub_i32_nominal_then_difference() {
    assert_eq!(
        crate::common::run_and_read_i32(&SUB_BYTECODE, 1, &[10, 3]),
        7
    );
}

// Overflow anchor: i32::MIN - 1 wraps to i32::MAX. A deterministic case pins
// the exact wrapping behaviour the random oracle only samples.
#[test]
fn execute_when_sub_i32_wraps_at_min_then_correct() {
    assert_eq!(
        crate::common::run_and_read_i32(&SUB_BYTECODE, 1, &[i32::MIN, 1]),
        i32::MAX
    );
}

// Trap anchor: SUB with an empty stack underflows.
#[test]
fn execute_when_sub_i32_stack_underflow_then_trap() {
    assert_eq!(
        crate::common::run_and_expect_trap_i32(&[0x24], 0, &[]),
        Trap::StackUnderflow
    );
}

// Property: cross-check SUB_I32 against Rust `wrapping_sub` over the full i32
// range. These tests already assert wrapping as ground truth, so the oracle is
// exact for every input pair.
proptest! {
    #[test]
    fn execute_when_sub_i32_any_then_matches_wrapping_sub(
        a in any::<i32>(),
        b in any::<i32>(),
    ) {
        let result = crate::common::run_and_read_i32(&SUB_BYTECODE, 1, &[a, b]);
        prop_assert_eq!(result, a.wrapping_sub(b));
    }
}
