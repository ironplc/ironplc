//! VM-specific edge case tests for boolean opcodes (BOOL_AND, BOOL_OR, BOOL_XOR, BOOL_NOT).
//!
//! Basic correctness is covered by end_to_end_bool.rs.
//! These tests cover non-zero integer coercion to boolean that cannot be
//! expressed in IEC 61131-3 source, plus a property test cross-checking every
//! boolean opcode against Rust's `(a != 0) op (b != 0)` coercion.

use proptest::prelude::*;

// Anchor: 5 AND 3 → 1 (both non-zero, coerced to true).
#[test]
fn execute_when_bool_and_nonzero_coercion_then_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (5)
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (3)
        0x78,              // BOOL_AND
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(crate::common::run_and_read_i32(&bytecode, 1, &[5, 3]), 1);
}

// Property: cross-check the boolean opcodes against Rust's coercion semantics,
// where any non-zero operand is `true` and the result is `1`/`0`.
proptest! {
    #[test]
    fn execute_when_bool_op_any_then_matches_rust(
        a in any::<i32>(),
        b in any::<i32>(),
        op in 0usize..4,
    ) {
        let (ba, bb) = (a != 0, b != 0);
        let (opcode, expected): (u8, i32) = match op {
            0 => (0x78, i32::from(ba && bb)), // BOOL_AND
            1 => (0x79, i32::from(ba || bb)), // BOOL_OR
            2 => (0x7A, i32::from(ba ^ bb)),  // BOOL_XOR
            _ => (0x7B, i32::from(!ba)),      // BOOL_NOT (unary)
        };
        let bytecode: Vec<u8> = if op == 3 {
            #[rustfmt::skip]
            let bc = vec![
                0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (a)
                0x7B,              // BOOL_NOT
                0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
                0x8C,              // RET_VOID
            ];
            bc
        } else {
            #[rustfmt::skip]
            let bc = vec![
                0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (a)
                0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (b)
                opcode,            // boolean op
                0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
                0x8C,              // RET_VOID
            ];
            bc
        };
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a, b]);
        prop_assert_eq!(result, expected);
    }
}
