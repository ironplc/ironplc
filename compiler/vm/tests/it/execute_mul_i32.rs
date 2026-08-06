//! VM-specific edge case tests for the MUL_I32 opcode.
//!
//! Basic correctness is covered by end_to_end_mul.rs.
//! These tests cover overflow wrapping and stack underflow traps that cannot be
//! expressed in IEC 61131-3 source, plus a property test that cross-checks the
//! opcode against Rust `wrapping_mul` over the full i32 range.

use ironplc_vm::error::Trap;
use proptest::prelude::*;

// LOAD_CONST_I32 pool[0], LOAD_CONST_I32 pool[1], MUL_I32, STORE_VAR_I32 var[0], RET_VOID
#[rustfmt::skip]
const MUL_BYTECODE: [u8; 11] = [
    0x00, 0x00, 0x00,
    0x00, 0x01, 0x00,
    0x28,
    0x10, 0x00, 0x00,
    0x8C,
];

// Nominal anchor: a straightforward product with no overflow.
#[test]
fn execute_when_mul_i32_nominal_then_product() {
    assert_eq!(
        crate::common::run_and_read_i32(&MUL_BYTECODE, 1, &[6, 7]),
        42
    );
}

// Overflow anchor: i32::MAX * 2 wraps to -2. A random oracle exercises this
// class, but keeping a deterministic case pins the exact wrapping behaviour.
#[test]
fn execute_when_mul_i32_max_times_two_then_wraps() {
    assert_eq!(
        crate::common::run_and_read_i32(&MUL_BYTECODE, 1, &[i32::MAX, 2]),
        -2
    );
}

// Trap anchor: MUL with an empty stack underflows.
#[test]
fn execute_when_mul_i32_stack_underflow_then_trap() {
    assert_eq!(
        crate::common::run_and_expect_trap_i32(&[0x28], 0, &[]),
        Trap::StackUnderflow
    );
}

// Property: cross-check MUL_I32 against Rust `wrapping_mul` over the full i32
// range. These tests already assert wrapping as ground truth, so the oracle is
// exact for every input pair.
proptest! {
    #[test]
    fn execute_when_mul_i32_any_then_matches_wrapping_mul(
        a in any::<i32>(),
        b in any::<i32>(),
    ) {
        let result = crate::common::run_and_read_i32(&MUL_BYTECODE, 1, &[a, b]);
        prop_assert_eq!(result, a.wrapping_mul(b));
    }
}
