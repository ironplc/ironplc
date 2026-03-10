//! End-to-end integration tests for LIMIT with ULINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_limit_ulint_in_range_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    result : ULINT;
  END_VAR
  result := LIMIT(ULINT#1000000000, ULINT#5000000000, ULINT#10000000000000000000);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64() as u64, 5_000_000_000);
}
