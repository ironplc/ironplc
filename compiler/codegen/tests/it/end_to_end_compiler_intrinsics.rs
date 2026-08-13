//! End-to-end tests for the `__TRUNC`/`__MOD` compiler intrinsics.
//!
//! These are the `__`-namespace intrinsics for real-number semantics IEC
//! 61131-3 source cannot express: truncation that stays in the real type,
//! and floating modulo (fmod, sign of the dividend). `ANY_REAL` genericity
//! is exercised at both widths — the op width selects the F32/F64 builtin
//! variant.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

/// Compiles a one-expression LREAL program and returns `result` (var 2).
///
/// Operands render via `{:?}` (uppercased), so values like `1.5e300` become
/// the ST exponent literal `1.5E300` instead of 300 digits of integer text.
fn eval_lreal(a: f64, b: f64, expression: &str) -> f64 {
    let source = format!(
        "PROGRAM main
VAR
    a : LREAL;
    b : LREAL;
    result : LREAL;
END_VAR
    a := {a};
    b := {b};
    result := {expression};
END_PROGRAM
",
        a = format!("{a:?}").to_uppercase(),
        b = format!("{b:?}").to_uppercase(),
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    bufs.vars[2].as_f64()
}

/// Compiles a one-expression REAL program and returns `result` (var 2).
fn eval_real(a: f32, b: f32, expression: &str) -> f32 {
    let source = format!(
        "PROGRAM main
VAR
    a : REAL;
    b : REAL;
    result : REAL;
END_VAR
    a := {a};
    b := {b};
    result := {expression};
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    bufs.vars[2].as_f32()
}

#[test]
fn end_to_end_when_trunc_intrinsic_lreal_then_truncates_toward_zero() {
    assert!((eval_lreal(3.7, 0.0, "__TRUNC(a)") - 3.0).abs() < 1e-9);
    assert!((eval_lreal(-3.7, 0.0, "__TRUNC(a)") + 3.0).abs() < 1e-9);
}

#[test]
fn end_to_end_when_trunc_intrinsic_lreal_beyond_integer_range_then_exact() {
    // The reason __TRUNC exists: unlike TRUNC (ANY_INT result), values
    // beyond any integer type's range truncate without clamping.
    let big = 1.5e300_f64;
    assert_eq!(eval_lreal(big, 0.0, "__TRUNC(a)"), big.trunc());
}

#[test]
fn end_to_end_when_trunc_intrinsic_real_then_truncates_toward_zero() {
    assert_eq!(eval_real(3.7, 0.0, "__TRUNC(a)"), 3.0);
    assert_eq!(eval_real(-3.7, 0.0, "__TRUNC(a)"), -3.0);
}

#[test]
fn end_to_end_when_mod_intrinsic_lreal_then_sign_of_dividend() {
    assert!((eval_lreal(400.56, 360.0, "__MOD(a, b)") - 40.56).abs() < 1e-9);
    assert!((eval_lreal(-400.56, 360.0, "__MOD(a, b)") + 40.56).abs() < 1e-9);
}

#[test]
fn end_to_end_when_mod_intrinsic_real_then_sign_of_dividend() {
    let r = eval_real(400.5, 360.0, "__MOD(a, b)");
    assert!((r - 40.5).abs() < 1e-4, "expected 40.5, got {r}");
    let r = eval_real(-400.5, 360.0, "__MOD(a, b)");
    assert!((r + 40.5).abs() < 1e-4, "expected -40.5, got {r}");
}

#[test]
fn end_to_end_when_mod_intrinsic_by_zero_then_nan_not_trap() {
    // Division by zero yields NaN — it must never trap the VM.
    assert!(eval_lreal(1.5, 0.0, "__MOD(a, b)").is_nan());
}

#[test]
fn end_to_end_when_intrinsics_composed_then_frac_shape_works() {
    // The composition Tc2_Math's FRAC will use: IN - __TRUNC(IN).
    assert!((eval_lreal(-3.7, 0.0, "a - __TRUNC(a)") + 0.7).abs() < 1e-9);
}
