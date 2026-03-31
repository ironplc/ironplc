//! End-to-end integration tests for MAX with LINT type.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_max_lint_first_larger_then_returns_first() {
    let source = "
PROGRAM main
  VAR
    a : LINT;
    b : LINT;
    result : LINT;
  END_VAR
  a := 10000000000;
  b := 5000000000;
  result := MAX(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i64(), 10_000_000_000);
}
