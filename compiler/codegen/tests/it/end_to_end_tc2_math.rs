//! End-to-end tests for the `Tc2_Math` compatibility library.
//!
//! Pins every test vector from the clean-room behavior specification
//! (`specs/design/library-interfaces/tc2-math.md`): the bundled library is
//! activated through the real registry, merged ahead of user source, analyzed,
//! compiled, and run on the VM. Rows the spec marks exact use exact equality;
//! the rest use the spec's `1.0E-9` epsilon; NaN rows assert `is_nan` and must
//! never trap.

use ironplc_analyzer::stages::analyze;
use ironplc_codegen::compile;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_sources::libraries::{remove_shadowed_functions, LibraryName, LibraryRegistry};
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::VmBuffers;

/// Activates the bundled `Tc2_Math`, analyzes it merged ahead of `source`,
/// compiles, and runs one scan cycle. The full activate → analyze → codegen →
/// VM-run path the acceptance criteria require.
fn run_with_tc2_math(source: &str) -> VmBuffers {
    let options = CompilerOptions::default();
    let compat = LibraryRegistry::bundled()
        .load(&LibraryName::from("Tc2_Math"))
        .expect("bundled Tc2_Math must load")
        .library;
    let user = parse_program(source, &FileId::default(), &options).unwrap();
    // The same user-shadowing filter the project pipeline applies.
    let compat = remove_shadowed_functions(vec![compat], &[&user]);
    let analyze_input: Vec<&Library> = compat.iter().chain(std::iter::once(&user)).collect();
    let (analyzed, context) = analyze(&analyze_input, &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );

    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: false,
    };
    let container = compile(
        &analyzed,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).expect("VM load must not trap");
        vm.run_round(0).expect("VM run must not trap");
    }
    bufs
}

/// Evaluates one library-function call over LREAL operands and returns
/// `result` (var 2).
///
/// Operands render via `{:?}` (uppercased), so values like `1.5e300` become
/// the ST exponent literal `1.5E300` instead of 300 digits of integer text.
fn eval(a: f64, b: f64, expression: &str) -> f64 {
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
    run_with_tc2_math(&source).vars[2].as_f64()
}

/// The spec's approximate-comparison epsilon.
fn assert_approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-9,
        "expected ≈ {expected}, got {actual}"
    );
}

// ---------------------------------------------------------------------------
// LTRUNC — integer part, staying LREAL (exact equality).
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_ltrunc_fractional_then_truncates_toward_zero() {
    assert_eq!(eval(2.8, 0.0, "LTRUNC(a)"), 2.0);
    assert_eq!(eval(-2.8, 0.0, "LTRUNC(a)"), -2.0);
    assert_eq!(eval(3.7, 0.0, "LTRUNC(a)"), 3.0);
    assert_eq!(eval(-3.7, 0.0, "LTRUNC(a)"), -3.0);
}

#[test]
fn end_to_end_when_ltrunc_integral_then_unchanged() {
    assert_eq!(eval(5.0, 0.0, "LTRUNC(a)"), 5.0);
}

#[test]
fn end_to_end_when_ltrunc_below_one_then_signed_zero() {
    assert_eq!(eval(0.9, 0.0, "LTRUNC(a)"), 0.0);
    let negative = eval(-0.9, 0.0, "LTRUNC(a)");
    assert_eq!(negative, 0.0);
    // The spec preserves the sign of zero: LTRUNC(-0.9) is -0.0.
    assert!(negative.is_sign_negative(), "expected -0.0, got {negative}");
}

#[test]
fn end_to_end_when_ltrunc_beyond_integer_range_then_exact_no_clamping() {
    // The reason LTRUNC exists: the result stays LREAL, so values beyond any
    // integer type's range truncate exactly.
    assert_eq!(eval(1.5e300, 0.0, "LTRUNC(a)"), 1.5e300);
    assert_eq!(eval(9.3e18, 0.0, "LTRUNC(a)"), 9.3e18);
}

// ---------------------------------------------------------------------------
// LMOD — floating modulo, signed remainder.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_lmod_positive_dividend_then_positive_remainder() {
    assert_approx(eval(400.56, 360.0, "LMOD(a, b)"), 40.56);
}

#[test]
fn end_to_end_when_lmod_negative_dividend_then_negative_remainder() {
    assert_approx(eval(-400.56, 360.0, "LMOD(a, b)"), -40.56);
}

#[test]
fn end_to_end_when_lmod_negative_divisor_then_sign_of_dividend() {
    assert_approx(eval(400.56, -360.0, "LMOD(a, b)"), 40.56);
}

#[test]
fn end_to_end_when_lmod_divides_evenly_then_exact_zero() {
    assert_eq!(eval(7.0, 3.5, "LMOD(a, b)"), 0.0);
}

#[test]
fn end_to_end_when_lmod_by_zero_then_nan_not_trap() {
    assert!(eval(1.5, 0.0, "LMOD(a, b)").is_nan());
}

// ---------------------------------------------------------------------------
// MODABS — modulo, unsigned result in [0, |IM|).
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_modabs_positive_then_same_as_lmod() {
    assert_approx(eval(400.56, 360.0, "MODABS(a, b)"), 40.56);
}

#[test]
fn end_to_end_when_modabs_negative_then_wraps_into_range() {
    assert_approx(eval(-400.56, 360.0, "MODABS(a, b)"), 319.44);
}

#[test]
fn end_to_end_when_modabs_negative_modulo_then_uses_magnitude() {
    assert_approx(eval(-400.56, -360.0, "MODABS(a, b)"), 319.44);
}

#[test]
fn end_to_end_when_modabs_multiple_then_exact_zero() {
    assert_eq!(eval(720.0, 360.0, "MODABS(a, b)"), 0.0);
}

#[test]
fn end_to_end_when_modabs_negative_multiple_then_zero_never_the_range_bound() {
    // The naive unconditional `LMOD + |IM|` composition would return 360.0
    // here (outside [0, 360)); the required conditional returns 0.0.
    let result = eval(-360.0, 360.0, "MODABS(a, b)");
    assert_eq!(result, 0.0);
    assert!(result.abs() < 360.0, "result must never equal |IM|");
}

#[test]
fn end_to_end_when_modabs_by_zero_then_nan_not_trap() {
    assert!(eval(1.5, 0.0, "MODABS(a, b)").is_nan());
}

// ---------------------------------------------------------------------------
// FRAC — fractional part.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_frac_positive_then_positive_fraction() {
    assert_approx(eval(2.8, 0.0, "FRAC(a)"), 0.8);
    assert_approx(eval(3.7, 0.0, "FRAC(a)"), 0.7);
}

#[test]
fn end_to_end_when_frac_negative_then_sign_of_input() {
    assert_approx(eval(-2.8, 0.0, "FRAC(a)"), -0.8);
    assert_approx(eval(-3.7, 0.0, "FRAC(a)"), -0.7);
}

#[test]
fn end_to_end_when_frac_integral_then_exact_zero() {
    assert_eq!(eval(5.0, 0.0, "FRAC(a)"), 0.0);
}

// ---------------------------------------------------------------------------
// Shadowing — a user-defined LTRUNC takes precedence over the library's.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_user_function_shadows_ltrunc_then_user_body_runs() {
    let bufs = run_with_tc2_math(
        "FUNCTION LTRUNC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
    LTRUNC := 123.0;
END_FUNCTION
PROGRAM main
VAR
    shadowed : LREAL;
    library_result : LREAL;
    a : LREAL;
    b : LREAL;
END_VAR
    a := -400.56;
    b := 360.0;
    shadowed := LTRUNC(2.8);
    library_result := MODABS(a, b);
END_PROGRAM
",
    );
    // The user's body (constant 123.0), not the library's truncation (2.0).
    assert_eq!(bufs.vars[0].as_f64(), 123.0);
    // The rest of the library remains active alongside the user override.
    assert!((bufs.vars[1].as_f64() - 319.44).abs() < 1.0e-9);
}
