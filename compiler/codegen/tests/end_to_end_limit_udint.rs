//! End-to-end integration tests for LIMIT with UDINT type.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32() as u32, 3_000_000_000);
}
