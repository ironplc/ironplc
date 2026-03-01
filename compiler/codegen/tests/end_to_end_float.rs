//! End-to-end integration tests for REAL (f32) and LREAL (f64) floating-point types.

mod common;

use common::parse_and_run;

// --- REAL (f32) tests ---

#[test]
fn end_to_end_when_real_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
  END_VAR
  x := 3.14;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let x = bufs.vars[0].as_f32();
    assert!((x - 3.14_f32).abs() < 1e-5, "expected ~3.14, got {x}");
}

#[test]
fn end_to_end_when_real_addition_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 1.5;
  y := x + 2.5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - 4.0).abs() < 1e-5, "expected 4.0, got {y}");
}

#[test]
fn end_to_end_when_real_subtraction_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 10.0;
  y := x - 3.5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - 6.5).abs() < 1e-5, "expected 6.5, got {y}");
}

#[test]
fn end_to_end_when_real_multiplication_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 2.5;
  y := x * 4.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - 10.0).abs() < 1e-5, "expected 10.0, got {y}");
}

#[test]
fn end_to_end_when_real_division_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 7.0;
  y := x / 2.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - 3.5).abs() < 1e-5, "expected 3.5, got {y}");
}

#[test]
fn end_to_end_when_real_negation_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 5.0;
  y := -x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - (-5.0)).abs() < 1e-5, "expected -5.0, got {y}");
}

#[test]
fn end_to_end_when_real_comparison_gt_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    result : DINT;
  END_VAR
  x := 5.0;
  y := 3.0;
  IF x > y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_real_comparison_eq_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    result : DINT;
  END_VAR
  x := 3.0;
  y := 3.0;
  IF x = y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_real_comparison_lt_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    result : DINT;
  END_VAR
  x := 2.0;
  y := 5.0;
  IF x < y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_real_comparison_le_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    result : DINT;
  END_VAR
  x := 3.0;
  y := 3.0;
  IF x <= y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_real_comparison_ne_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    result : DINT;
  END_VAR
  x := 3.0;
  y := 4.0;
  IF x <> y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_real_comparison_ge_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
    result : DINT;
  END_VAR
  x := 5.0;
  y := 5.0;
  IF x >= y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_real_integer_literal_then_converts() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
  END_VAR
  x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let x = bufs.vars[0].as_f32();
    assert!((x - 42.0).abs() < 1e-5, "expected 42.0, got {x}");
}

#[test]
fn end_to_end_when_real_expt_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 3.0;
  y := x ** 2.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - 9.0).abs() < 1e-3, "expected 9.0, got {y}");
}

// --- LREAL (f64) tests ---

#[test]
fn end_to_end_when_lreal_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
  END_VAR
  x := 3.141592653589793;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let x = bufs.vars[0].as_f64();
    assert!(
        (x - 3.141592653589793_f64).abs() < 1e-12,
        "expected pi, got {x}"
    );
}

#[test]
fn end_to_end_when_lreal_addition_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.5;
  y := x + 2.5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!((y - 4.0).abs() < 1e-12, "expected 4.0, got {y}");
}

#[test]
fn end_to_end_when_lreal_subtraction_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 10.0;
  y := x - 3.5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!((y - 6.5).abs() < 1e-12, "expected 6.5, got {y}");
}

#[test]
fn end_to_end_when_lreal_multiplication_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 2.5;
  y := x * 4.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!((y - 10.0).abs() < 1e-12, "expected 10.0, got {y}");
}

#[test]
fn end_to_end_when_lreal_division_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 7.0;
  y := x / 2.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!((y - 3.5).abs() < 1e-12, "expected 3.5, got {y}");
}

#[test]
fn end_to_end_when_lreal_negation_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 5.0;
  y := -x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!((y - (-5.0)).abs() < 1e-12, "expected -5.0, got {y}");
}

#[test]
fn end_to_end_when_lreal_comparison_gt_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
    result : DINT;
  END_VAR
  x := 5.0;
  y := 3.0;
  IF x > y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_lreal_comparison_lt_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
    result : DINT;
  END_VAR
  x := 2.0;
  y := 5.0;
  IF x < y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_lreal_comparison_eq_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
    result : DINT;
  END_VAR
  x := 3.0;
  y := 3.0;
  IF x = y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_lreal_comparison_ne_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
    result : DINT;
  END_VAR
  x := 3.0;
  y := 4.0;
  IF x <> y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_lreal_comparison_le_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
    result : DINT;
  END_VAR
  x := 3.0;
  y := 3.0;
  IF x <= y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_lreal_comparison_ge_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
    result : DINT;
  END_VAR
  x := 5.0;
  y := 5.0;
  IF x >= y THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_lreal_precision_then_exceeds_f32() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
  END_VAR
  x := 1.0000000000000002;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // This value is distinguishable from 1.0 in f64 but not in f32
    let x = bufs.vars[0].as_f64();
    assert!(x != 1.0_f64, "expected value distinct from 1.0 in f64");
}

#[test]
fn end_to_end_when_lreal_expt_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 2.0;
  y := x ** 10.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!((y - 1024.0).abs() < 1e-6, "expected 1024.0, got {y}");
}
