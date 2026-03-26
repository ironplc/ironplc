//! End-to-end integration tests for MIN with UDINT type.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_min_udint_large_values_then_unsigned_comparison() {
    let source = "
PROGRAM main
  VAR
    a : UDINT;
    b : UDINT;
    result : UDINT;
  END_VAR
  a := 3000000000;
  b := 1000000000;
  result := MIN(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[2].as_i32() as u32, 1_000_000_000);
}
