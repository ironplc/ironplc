//! Integration tests for f64 arithmetic opcodes.
//!
//! Deterministic anchors pin the non-finite behaviours a random oracle cannot
//! check (div-by-zero → ±∞, NaN propagation, negation). A property test
//! cross-checks the binary opcodes against Rust `+ - * /` over finite inputs.

use proptest::prelude::*;

// Anchor: 1.0 / 0.0 → +∞ (a random oracle over finite inputs can't reach this).
#[test]
fn execute_when_div_f64_by_zero_then_positive_infinity() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (1.0)
        0x03, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (0.0)
        0x33,              // DIV_F64
        0x13, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0x8C,              // RET_VOID
    ];
    let result = crate::common::run_and_read_f64(&bytecode, 1, &[1.0, 0.0]);
    assert!(result.is_infinite() && result.is_sign_positive());
}

// Anchor: NaN propagates through arithmetic.
#[test]
fn execute_when_sub_f64_nan_then_nan() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (NaN)
        0x03, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (5.0)
        0x27,              // SUB_F64
        0x13, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0x8C,              // RET_VOID
    ];
    let result = crate::common::run_and_read_f64(&bytecode, 1, &[f64::NAN, 5.0]);
    assert!(result.is_nan());
}

// Anchor: NEG_F64 negates (not exercised by the binary-op property test).
#[test]
fn execute_when_neg_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (42.5)
        0x2F,              // NEG_F64
        0x13, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0x8C,              // RET_VOID
    ];
    let result = crate::common::run_and_read_f64(&bytecode, 1, &[42.5]);
    assert_eq!(result, -42.5);
}

// Property: cross-check ADD/SUB/MUL/DIV_F64 against Rust arithmetic. Inputs are
// finite so the oracle is well-defined; overflow to ±∞ and 0/0 → NaN are still
// possible results, so NaN is compared structurally.
proptest! {
    #[test]
    fn execute_when_arith_f64_finite_then_matches_rust(
        a in any::<f64>().prop_filter("finite", |v| v.is_finite()),
        b in any::<f64>().prop_filter("finite", |v| v.is_finite()),
        op in 0usize..4,
    ) {
        let (opcode, expected): (u8, f64) = match op {
            0 => (0x23, a + b), // ADD_F64
            1 => (0x27, a - b), // SUB_F64
            2 => (0x2B, a * b), // MUL_F64
            _ => (0x33, a / b), // DIV_F64
        };
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x03, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (a)
            0x03, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (b)
            opcode,            // arithmetic
            0x13, 0x00, 0x00,  // STORE_VAR_F64 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_f64(&bytecode, 1, &[a, b]);
        if expected.is_nan() {
            prop_assert!(result.is_nan());
        } else {
            prop_assert_eq!(result, expected);
        }
    }
}
