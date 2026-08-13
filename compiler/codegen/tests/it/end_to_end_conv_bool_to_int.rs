//! End-to-end tests for BOOL to integer type conversions.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_run;

/// BOOL_TO_<T> for targets that fit in a 32-bit slot. TRUE -> 1, FALSE -> 0.
#[rstest]
#[case::sint_true("SINT", "TRUE", 1)]
#[case::sint_false("SINT", "FALSE", 0)]
#[case::int_true("INT", "TRUE", 1)]
#[case::int_false("INT", "FALSE", 0)]
#[case::dint_true("DINT", "TRUE", 1)]
#[case::usint_true("USINT", "TRUE", 1)]
#[case::uint_true("UINT", "TRUE", 1)]
#[case::udint_true("UDINT", "TRUE", 1)]
fn end_to_end_bool_to_int32(#[case] target: &str, #[case] input: &str, #[case] expected: i32) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : BOOL;
    y : {target};
  END_VAR
  x := {input};
  y := BOOL_TO_{target}(x);
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

/// BOOL_TO_<T> for 64-bit targets. TRUE -> 1, FALSE -> 0.
#[rstest]
#[case::lint_true("LINT", "TRUE", 1)]
#[case::ulint_true("ULINT", "TRUE", 1)]
#[case::ulint_false("ULINT", "FALSE", 0)]
fn end_to_end_bool_to_int64(#[case] target: &str, #[case] input: &str, #[case] expected: i64) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : BOOL;
    y : {target};
  END_VAR
  x := {input};
  y := BOOL_TO_{target}(x);
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i64(), expected);
}
