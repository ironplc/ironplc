//! End-to-end tests for integer narrowing type conversions.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_run;

/// <SRC>_TO_<TGT> narrowing conversions. The overflow case wraps into the
/// target width (300 truncated to SINT = 300 mod 256 = 44).
#[rstest]
#[case::dint_to_int("DINT", "INT", "1000", 1000)]
#[case::lint_to_dint("LINT", "DINT", "42", 42)]
#[case::dint_to_sint_overflow("DINT", "SINT", "300", 44)]
#[case::lint_to_sint("LINT", "SINT", "50", 50)]
#[case::ulint_to_udint("ULINT", "UDINT", "1000", 1000)]
fn end_to_end_int_narrowing(
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
