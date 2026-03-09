//! End-to-end integration tests for LIMIT with LINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_limit_lint_in_range_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := LIMIT(LINT#-10000000000, LINT#5000000000, LINT#10000000000);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 5_000_000_000);
}

#[test]
fn end_to_end_when_limit_lint_below_min_then_clamped() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := LIMIT(LINT#0, LINT#-5000000000, LINT#10000000000);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 0);
}
