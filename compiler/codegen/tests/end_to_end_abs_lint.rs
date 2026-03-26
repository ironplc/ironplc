//! End-to-end integration tests for ABS with LINT type.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_abs_lint_negative_then_positive() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := -7000000000;
  y := ABS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[1].as_i64(), 7_000_000_000);
}
