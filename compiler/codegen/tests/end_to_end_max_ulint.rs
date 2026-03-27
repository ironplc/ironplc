//! End-to-end integration tests for MAX with ULINT type.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_max_ulint_large_values_then_unsigned_comparison() {
    let source = "
PROGRAM main
  VAR
    a : ULINT;
    b : ULINT;
    result : ULINT;
  END_VAR
  a := 10000000000000000000;
  b := 5000000000;
  result := MAX(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i64() as u64, 10_000_000_000_000_000_000);
}
