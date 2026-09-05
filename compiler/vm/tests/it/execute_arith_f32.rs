//! Integration tests for f32 arithmetic opcodes.
//!
//! Deterministic anchors pin the non-finite behaviours a random oracle cannot
//! check (div-by-zero → ±∞, NaN propagation, negation). A property test
//! cross-checks the binary opcodes against Rust `+ - * /` over finite inputs.

use proptest::prelude::*;

// Anchor: 1.0 / 0.0 → +∞ (a random oracle over finite inputs can't reach this).
#[test]
fn execute_when_div_f32_by_zero_then_positive_infinity() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (1.0)
        0x02, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]  (0.0)
        0x32,              // DIV_F32
        0x12, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0x8C,              // RET_VOID
    ];
    let result = crate::common::run_and_read_f32(&bytecode, 1, &[1.0, 0.0]);
    assert!(result.is_infinite() && result.is_sign_positive());
}

// Anchor: NaN propagates through arithmetic.
#[test]
fn execute_when_add_f32_nan_then_nan() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (NaN)
        0x02, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]  (1.0)
        0x22,              // ADD_F32
        0x12, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0x8C,              // RET_VOID
    ];
    let result = crate::common::run_and_read_f32(&bytecode, 1, &[f32::NAN, 1.0]);
    assert!(result.is_nan());
}

// Anchor: NEG_F32 negates (not exercised by the binary-op property test).
#[test]
fn execute_when_neg_f32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (5.5)
        0x2E,              // NEG_F32
        0x12, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0x8C,              // RET_VOID
    ];
    let result = crate::common::run_and_read_f32(&bytecode, 1, &[5.5]);
    assert_eq!(result, -5.5);
}

// Property: cross-check ADD/SUB/MUL/DIV_F32 against Rust arithmetic. Inputs are
// finite so the oracle is well-defined; overflow to ±∞ and 0/0 → NaN are still
// possible results, so NaN is compared structurally.
proptest! {
    #[test]
    fn execute_when_arith_f32_finite_then_matches_rust(
        a in any::<f32>().prop_filter("finite", |v| v.is_finite()),
        b in any::<f32>().prop_filter("finite", |v| v.is_finite()),
        op in 0usize..4,
    ) {
        let (opcode, expected): (u8, f32) = match op {
            0 => (0x22, a + b), // ADD_F32
            1 => (0x26, a - b), // SUB_F32
            2 => (0x2A, a * b), // MUL_F32
            _ => (0x32, a / b), // DIV_F32
        };
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x02, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (a)
            0x02, 0x01, 0x00,  // LOAD_CONST_F32 pool[1] (b)
            opcode,            // arithmetic
            0x12, 0x00, 0x00,  // STORE_VAR_F32 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_f32(&bytecode, 1, &[a, b]);
        if expected.is_nan() {
            prop_assert!(result.is_nan());
        } else {
            prop_assert_eq!(result, expected);
        }
    }
}
