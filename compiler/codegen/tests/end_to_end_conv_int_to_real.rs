//! End-to-end tests for integer-to-real type conversions.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_int_to_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : REAL;
  END_VAR
  x := 42;
  y := INT_TO_REAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[1].as_f32() - 42.0).abs() < 1e-5);
}

#[test]
fn end_to_end_when_dint_to_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : LREAL;
  END_VAR
  x := -100;
  y := DINT_TO_LREAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[1].as_f64() - (-100.0)).abs() < 1e-12);
}

#[test]
fn end_to_end_when_sint_to_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
    y : REAL;
  END_VAR
  x := -7;
  y := SINT_TO_REAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[1].as_f32() - (-7.0)).abs() < 1e-5);
}

#[test]
fn end_to_end_when_lint_to_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LREAL;
  END_VAR
  x := 123456789;
  y := LINT_TO_LREAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[1].as_f64() - 123456789.0).abs() < 1.0);
}

#[test]
fn end_to_end_when_uint_to_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
    y : REAL;
  END_VAR
  x := 40000;
  y := UINT_TO_REAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[1].as_f32() - 40000.0).abs() < 1.0);
}
