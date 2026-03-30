//! End-to-end integration tests for MIN with LINT type.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_min_lint_then_returns_smaller() {
    let source = "
PROGRAM main
  VAR
    a : LINT;
    b : LINT;
    result : LINT;
  END_VAR
  a := -5000000000;
  b := 3000000000;
  result := MIN(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i64(), -5_000_000_000);
}
