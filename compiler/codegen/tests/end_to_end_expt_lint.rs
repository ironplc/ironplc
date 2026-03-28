//! End-to-end integration tests for EXPT with LINT type.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_expt_lint_then_correct() {
    let source = "
PROGRAM main
  VAR
    base : LINT;
    exp : LINT;
    result : LINT;
  END_VAR
  base := 2;
  exp := 40;
  result := EXPT(base, exp);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i64(), 1_099_511_627_776);
}
