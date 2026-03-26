//! End-to-end integration tests for LN, LOG, EXP functions.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_ln_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 2.718282;
  y := LN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 1.0).abs() < 1e-4, "expected ~1.0, got {y}");
}

#[test]
fn end_to_end_when_ln_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.0;
  y := LN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!((y - 0.0).abs() < 1e-12, "expected 0.0, got {y}");
}

#[test]
fn end_to_end_when_log_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 100.0;
  y := LOG(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 2.0).abs() < 1e-5, "expected 2.0, got {y}");
}

#[test]
fn end_to_end_when_log_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1000.0;
  y := LOG(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!((y - 3.0).abs() < 1e-12, "expected 3.0, got {y}");
}

#[test]
fn end_to_end_when_exp_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := EXP(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 1.0).abs() < 1e-5, "expected 1.0, got {y}");
}

#[test]
fn end_to_end_when_exp_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.0;
  y := EXP(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!(
        (y - std::f64::consts::E).abs() < 1e-12,
        "expected E, got {y}"
    );
}
