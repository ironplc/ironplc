//! End-to-end integration tests for the SEL function with float types.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_sel_real_false_then_returns_in0() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := SEL(0, 10.5, 20.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[0].as_f32();
    assert!((y - 10.5).abs() < 1e-5, "expected 10.5, got {y}");
}

#[test]
fn end_to_end_when_sel_real_true_then_returns_in1() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := SEL(1, 10.5, 20.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[0].as_f32();
    assert!((y - 20.5).abs() < 1e-5, "expected 20.5, got {y}");
}

#[test]
fn end_to_end_when_sel_real_with_variable_then_selects() {
    let source = "
PROGRAM main
  VAR
    g : DINT;
    y : REAL;
  END_VAR
  g := 1;
  y := SEL(g, 100.0, 200.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 200.0).abs() < 1e-5, "expected 200.0, got {y}");
}

#[test]
fn end_to_end_when_sel_lreal_false_then_returns_in0() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := SEL(0, 10.5, 20.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[0].as_f64();
    assert!((y - 10.5).abs() < 1e-12, "expected 10.5, got {y}");
}

#[test]
fn end_to_end_when_sel_lreal_true_then_returns_in1() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := SEL(1, 10.5, 20.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[0].as_f64();
    assert!((y - 20.5).abs() < 1e-12, "expected 20.5, got {y}");
}
