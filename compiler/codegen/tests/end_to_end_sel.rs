//! End-to-end integration tests for the SEL function.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_sel_false_then_returns_in0() {
    let source = "
PROGRAM main
  VAR
    y : INT;
  END_VAR
  y := SEL(0, 10, 20);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 10);
}

#[test]
fn end_to_end_when_sel_true_then_returns_in1() {
    let source = "
PROGRAM main
  VAR
    y : INT;
  END_VAR
  y := SEL(1, 10, 20);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 20);
}

#[test]
fn end_to_end_when_sel_with_variable_then_selects() {
    let source = "
PROGRAM main
  VAR
    g : INT;
    y : INT;
  END_VAR
  g := 1;
  y := SEL(g, 100, 200);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 200);
}
