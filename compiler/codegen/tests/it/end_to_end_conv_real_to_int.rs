//! End-to-end tests for real-to-integer type conversions.
//!
//! These stay parametrized tables rather than property tests: IEC truncation
//! semantics do not map cleanly onto a single Rust `as` cast oracle.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_run;

/// REAL/LREAL -> integer conversions that read from a 32-bit slot. Fractional
/// parts are truncated toward zero.
#[rstest]
#[case::real_to_int("REAL", "INT", "3.14", 3)]
#[case::real_to_dint_negative("REAL", "DINT", "-7.9", -7)]
#[case::real_to_sint("REAL", "SINT", "50.7", 50)]
#[case::lreal_to_udint("LREAL", "UDINT", "1000.0", 1000)]
fn end_to_end_real_to_int_i32(
    #[case] src: &str,
    #[case] tgt: &str,
    #[case] value: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : {src};
    y : {tgt};
  END_VAR
  x := {value};
  y := {src}_TO_{tgt}(x);
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

/// REAL/LREAL -> integer conversions that read from a 64-bit slot.
#[rstest]
#[case::lreal_to_lint("LREAL", "LINT", "99.9", 99)]
fn end_to_end_real_to_int_i64(
    #[case] src: &str,
    #[case] tgt: &str,
    #[case] value: &str,
    #[case] expected: i64,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : {src};
    y : {tgt};
  END_VAR
  x := {value};
  y := {src}_TO_{tgt}(x);
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i64(), expected);
}
