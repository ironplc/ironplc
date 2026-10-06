//! End-to-end integration tests for LIMIT with UDINT type.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

#[test]
fn end_to_end_when_limit_udint_above_max_then_clamped() {
    let source = "
PROGRAM main
  VAR
    result : UDINT;
  END_VAR
  result := LIMIT(1000000000, 4000000000, 3000000000);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<u32>("result"), 3_000_000_000);
}
