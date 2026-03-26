//! End-to-end integration tests for SEL with LINT type.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_sel_lint_false_then_returns_in0() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := SEL(0, LINT#5000000000, LINT#10000000000);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[0].as_i64(), 5_000_000_000);
}

#[test]
fn end_to_end_when_sel_lint_true_then_returns_in1() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := SEL(1, LINT#5000000000, LINT#10000000000);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[0].as_i64(), 10_000_000_000);
}
