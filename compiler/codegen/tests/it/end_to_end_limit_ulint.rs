//! End-to-end integration tests for LIMIT with ULINT type.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

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
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<u64>("result"), 5_000_000_000);
}
