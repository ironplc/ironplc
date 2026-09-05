//! VM tests for the unnamed TRUNC_F64 and MOD_F64 builtins.
//!
//! These func_ids have no compiler-seeded name — they are not reachable from
//! IEC 61131-3 source yet (a follow-up binds them through the compatibility
//! library mechanism), so their semantics are pinned here at the VM level:
//! truncation toward zero, and fmod with the sign of the dividend where
//! division by zero yields NaN rather than a trap.

/// Bytecode: load pool[0], BUILTIN TRUNC_F64, store var[0].
fn trunc_bytecode() -> Vec<u8> {
    #[rustfmt::skip]
    let bytecode = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0x94, 0xA3, 0x03,  // BUILTIN TRUNC_F64 (0x03A3)
        0x13, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0x8C,              // RET_VOID
    ];
    bytecode
}

/// Bytecode: load pool[0] (dividend), load pool[1] (divisor),
/// BUILTIN MOD_F64, store var[0].
fn fmod_bytecode() -> Vec<u8> {
    #[rustfmt::skip]
    let bytecode = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0x03, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]
        0x94, 0xA4, 0x03,  // BUILTIN MOD_F64 (0x03A4)
        0x13, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0x8C,              // RET_VOID
    ];
    bytecode
}

#[test]
fn execute_when_trunc_f64_positive_then_truncates_toward_zero() {
    assert_eq!(
        crate::common::run_and_read_f64(&trunc_bytecode(), 1, &[3.7]),
        3.0
    );
}

#[test]
fn execute_when_trunc_f64_negative_then_truncates_toward_zero() {
    assert_eq!(
        crate::common::run_and_read_f64(&trunc_bytecode(), 1, &[-3.7]),
        -3.0
    );
}

#[test]
fn execute_when_trunc_f64_beyond_i64_range_then_stays_lreal() {
    // The reason this builtin exists: values outside any integer type's
    // range truncate without clamping.
    let big = 1.5e300_f64;
    assert_eq!(
        crate::common::run_and_read_f64(&trunc_bytecode(), 1, &[big]),
        big.trunc()
    );
}

#[test]
fn execute_when_mod_f64_positive_then_fractional_remainder() {
    let r = crate::common::run_and_read_f64(&fmod_bytecode(), 1, &[400.56, 360.0]);
    assert!(
        (r - 40.56).abs() < 1e-9,
        "fmod(400.56, 360) = 40.56, got {r}"
    );
}

#[test]
fn execute_when_mod_f64_negative_dividend_then_sign_of_dividend() {
    let r = crate::common::run_and_read_f64(&fmod_bytecode(), 1, &[-400.56, 360.0]);
    assert!(
        (r + 40.56).abs() < 1e-9,
        "fmod(-400.56, 360) = -40.56, got {r}"
    );
}

#[test]
fn execute_when_mod_f64_negative_divisor_then_sign_of_dividend() {
    let r = crate::common::run_and_read_f64(&fmod_bytecode(), 1, &[400.56, -360.0]);
    assert!(
        (r - 40.56).abs() < 1e-9,
        "fmod keeps the dividend's sign, got {r}"
    );
}

#[test]
fn execute_when_mod_f64_by_zero_then_nan_not_trap() {
    // Division by zero must produce NaN, never a trap.
    let r = crate::common::run_and_read_f64(&fmod_bytecode(), 1, &[1.5, 0.0]);
    assert!(r.is_nan(), "fmod by zero must be NaN, got {r}");
}
