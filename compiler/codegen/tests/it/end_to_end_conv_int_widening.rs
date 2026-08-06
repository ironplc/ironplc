//! End-to-end tests for integer widening type conversions.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_run;

/// <SRC>_TO_<TGT> widening/reinterpret conversions read from a 32-bit slot.
#[rstest]
#[case::sint_to_int("SINT", "INT", "-100", -100)]
#[case::int_to_dint("INT", "DINT", "-30000", -30000)]
#[case::usint_to_uint("USINT", "UINT", "200", 200)]
#[case::int_to_uint("INT", "UINT", "1000", 1000)]
fn end_to_end_int_widening_i32(
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

/// <SRC>_TO_<TGT> widening conversions read from a 64-bit slot.
#[rstest]
#[case::dint_to_lint("DINT", "LINT", "-1000000", -1_000_000)]
#[case::uint_to_ulint("UINT", "ULINT", "50000", 50_000)]
fn end_to_end_int_widening_i64(
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
