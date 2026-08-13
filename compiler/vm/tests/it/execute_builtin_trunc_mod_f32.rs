//! VM tests for the TRUNC_F32 and MOD_F32 builtins — the f32 variants of
//! the `__TRUNC`/`__MOD` lowering targets (see
//! execute_builtin_trunc_mod_f64.rs for the f64 variants and the full
//! semantics: truncation toward zero, and fmod with the sign of the
//! dividend where division by zero yields NaN rather than a trap).

/// Bytecode: load pool[0], BUILTIN TRUNC_F32, store var[0].
fn trunc_bytecode() -> Vec<u8> {
    #[rustfmt::skip]
    let bytecode = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0x94, 0xA5, 0x03,  // BUILTIN TRUNC_F32 (0x03A5)
        0x12, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0x8C,              // RET_VOID
    ];
    bytecode
}

/// Bytecode: load pool[0] (dividend), load pool[1] (divisor),
/// BUILTIN MOD_F32, store var[0].
fn fmod_bytecode() -> Vec<u8> {
    #[rustfmt::skip]
    let bytecode = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0x02, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]
        0x94, 0xA6, 0x03,  // BUILTIN MOD_F32 (0x03A6)
        0x12, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0x8C,              // RET_VOID
    ];
    bytecode
}

#[test]
fn execute_when_trunc_f32_positive_then_truncates_toward_zero() {
    assert_eq!(
        crate::common::run_and_read_f32(&trunc_bytecode(), 1, &[3.7]),
        3.0
    );
}

#[test]
fn execute_when_trunc_f32_negative_then_truncates_toward_zero() {
    assert_eq!(
        crate::common::run_and_read_f32(&trunc_bytecode(), 1, &[-3.7]),
        -3.0
    );
}

#[test]
fn execute_when_mod_f32_positive_then_fractional_remainder() {
    let r = crate::common::run_and_read_f32(&fmod_bytecode(), 1, &[400.5, 360.0]);
    assert!((r - 40.5).abs() < 1e-4, "__MOD(400.5, 360) = 40.5, got {r}");
}

#[test]
fn execute_when_mod_f32_negative_dividend_then_sign_of_dividend() {
    let r = crate::common::run_and_read_f32(&fmod_bytecode(), 1, &[-400.5, 360.0]);
    assert!(
        (r + 40.5).abs() < 1e-4,
        "__MOD(-400.5, 360) = -40.5, got {r}"
    );
}

#[test]
fn execute_when_mod_f32_by_zero_then_nan_not_trap() {
    let r = crate::common::run_and_read_f32(&fmod_bytecode(), 1, &[1.5, 0.0]);
    assert!(r.is_nan(), "fmod by zero must be NaN, got {r}");
}
