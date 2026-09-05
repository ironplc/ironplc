//! Integration tests for bitwise opcodes (BIT_AND_32, BIT_OR_32, BIT_XOR_32,
//! BIT_NOT_32, BIT_AND_64, BIT_OR_64, BIT_XOR_64, BIT_NOT_64).
//!
//! One deterministic anchor per width pins a known bit pattern; property tests
//! cross-check every operator against Rust's `& | ^ !` over the full range.

use proptest::prelude::*;

// Anchor (32-bit): 0xFF AND 0x0F → 0x0F.
#[test]
fn execute_when_bit_and_32_then_bitwise_and() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0xFF = 255)
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (0x0F = 15)
        0x68,              // BIT_AND_32
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(
        crate::common::run_and_read_i32(&bytecode, 1, &[0xFF, 0x0F]),
        0x0F
    );
}

// Anchor (64-bit): 0xFF AND 0x0F → 0x0F.
#[test]
fn execute_when_bit_and_64_then_bitwise_and() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (0xFF)
        0x01, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (0x0F)
        0x69,              // BIT_AND_64
        0x11, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(
        crate::common::run_and_read_i64(&bytecode, 1, &[0xFF, 0x0F]),
        0x0F
    );
}

// Property: cross-check the 32-bit bitwise opcodes against Rust `& | ^ !`.
proptest! {
    #[test]
    fn execute_when_bitwise_32_any_then_matches_rust(
        a in any::<i32>(),
        b in any::<i32>(),
        op in 0usize..4,
    ) {
        let (opcode, expected): (u8, i32) = match op {
            0 => (0x68, a & b), // BIT_AND_32
            1 => (0x6C, a | b), // BIT_OR_32
            2 => (0x70, a ^ b), // BIT_XOR_32
            _ => (0x74, !a),    // BIT_NOT_32 (unary)
        };
        let bytecode: Vec<u8> = if op == 3 {
            #[rustfmt::skip]
            let bc = vec![
                0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (a)
                0x74,              // BIT_NOT_32
                0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
                0x8C,              // RET_VOID
            ];
            bc
        } else {
            #[rustfmt::skip]
            let bc = vec![
                0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (a)
                0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (b)
                opcode,            // bitwise op
                0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
                0x8C,              // RET_VOID
            ];
            bc
        };
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a, b]);
        prop_assert_eq!(result, expected);
    }

    #[test]
    fn execute_when_bitwise_64_any_then_matches_rust(
        a in any::<i64>(),
        b in any::<i64>(),
        op in 0usize..4,
    ) {
        let (opcode, expected): (u8, i64) = match op {
            0 => (0x69, a & b), // BIT_AND_64
            1 => (0x6D, a | b), // BIT_OR_64
            2 => (0x71, a ^ b), // BIT_XOR_64
            _ => (0x75, !a),    // BIT_NOT_64 (unary)
        };
        let bytecode: Vec<u8> = if op == 3 {
            #[rustfmt::skip]
            let bc = vec![
                0x01, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (a)
                0x75,              // BIT_NOT_64
                0x11, 0x00, 0x00,  // STORE_VAR_I64 var[0]
                0x8C,              // RET_VOID
            ];
            bc
        } else {
            #[rustfmt::skip]
            let bc = vec![
                0x01, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (a)
                0x01, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (b)
                opcode,            // bitwise op
                0x11, 0x00, 0x00,  // STORE_VAR_I64 var[0]
                0x8C,              // RET_VOID
            ];
            bc
        };
        let result = crate::common::run_and_read_i64(&bytecode, 1, &[a, b]);
        prop_assert_eq!(result, expected);
    }
}
