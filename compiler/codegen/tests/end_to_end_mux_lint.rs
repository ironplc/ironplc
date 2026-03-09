//! End-to-end integration tests for MUX with LINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_mux_lint_k0_then_returns_in0() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := MUX(0, LINT#5000000000, LINT#10000000000);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 5_000_000_000);
}

#[test]
fn end_to_end_when_mux_lint_k1_then_returns_in1() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := MUX(1, LINT#5000000000, LINT#10000000000);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 10_000_000_000);
}

#[test]
fn end_to_end_when_mux_lint_k2_3_inputs_then_returns_in2() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := MUX(2, LINT#100, LINT#200, LINT#300);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 300);
}

#[test]
fn end_to_end_when_mux_lint_k_out_of_range_then_clamps_to_last() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
  END_VAR
  result := MUX(10, LINT#100, LINT#200, LINT#300);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 300);
}

#[test]
fn end_to_end_when_mux_lint_k_negative_then_clamps_to_first() {
    let source = "
PROGRAM main
  VAR
    k : DINT;
    result : LINT;
  END_VAR
  k := -1;
  result := MUX(k, LINT#100, LINT#200);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 100);
}
