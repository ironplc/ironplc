//! End-to-end integration tests for SIN, COS, TAN, ASIN, ACOS, ATAN functions.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_sin_real_zero_then_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := SIN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 0.0).abs() < 1e-5, "expected 0.0, got {y}");
}

#[test]
fn end_to_end_when_sin_lreal_pi_half_then_one() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.5707963267948966;
  y := SIN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!((y - 1.0).abs() < 1e-12, "expected 1.0, got {y}");
}

#[test]
fn end_to_end_when_cos_real_zero_then_one() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := COS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 1.0).abs() < 1e-5, "expected 1.0, got {y}");
}

#[test]
fn end_to_end_when_cos_lreal_pi_then_neg_one() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 3.141592653589793;
  y := COS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!((y - (-1.0)).abs() < 1e-12, "expected -1.0, got {y}");
}

#[test]
fn end_to_end_when_tan_real_zero_then_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := TAN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 0.0).abs() < 1e-5, "expected 0.0, got {y}");
}

#[test]
fn end_to_end_when_tan_lreal_pi_quarter_then_one() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 0.7853981633974483;
  y := TAN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!((y - 1.0).abs() < 1e-12, "expected 1.0, got {y}");
}

#[test]
fn end_to_end_when_asin_real_zero_then_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := ASIN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 0.0).abs() < 1e-5, "expected 0.0, got {y}");
}

#[test]
fn end_to_end_when_asin_lreal_one_then_pi_half() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.0;
  y := ASIN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!(
        (y - std::f64::consts::FRAC_PI_2).abs() < 1e-12,
        "expected PI/2, got {y}"
    );
}

#[test]
fn end_to_end_when_acos_real_one_then_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 1.0;
  y := ACOS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 0.0).abs() < 1e-5, "expected 0.0, got {y}");
}

#[test]
fn end_to_end_when_acos_lreal_zero_then_pi_half() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 0.0;
  y := ACOS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!(
        (y - std::f64::consts::FRAC_PI_2).abs() < 1e-12,
        "expected PI/2, got {y}"
    );
}

#[test]
fn end_to_end_when_atan_real_zero_then_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := ATAN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 0.0).abs() < 1e-5, "expected 0.0, got {y}");
}

#[test]
fn end_to_end_when_atan_lreal_one_then_pi_quarter() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.0;
  y := ATAN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!(
        (y - std::f64::consts::FRAC_PI_4).abs() < 1e-12,
        "expected PI/4, got {y}"
    );
}
