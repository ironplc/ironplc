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

// --- IEEE 754 edge cases: Inf, NaN ---

#[test]
fn end_to_end_when_real_divide_by_zero_then_inf() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 1.0;
  y := x / 0.0;
END_PROGRAM
";
    // Float divide-by-zero does NOT trap — produces Inf per IEEE 754
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!(y.is_infinite() && y > 0.0, "expected +Inf, got {y}");
}

#[test]
fn end_to_end_when_real_negative_divide_by_zero_then_neg_inf() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := -1.0;
  y := x / 0.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!(y.is_infinite() && y < 0.0, "expected -Inf, got {y}");
}

#[test]
fn end_to_end_when_real_zero_divide_by_zero_then_nan() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := x / 0.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!(y.is_nan(), "expected NaN, got {y}");
}

#[test]
fn end_to_end_when_real_nan_comparison_then_all_false() {
    // IEEE 754: NaN is not equal to anything, including itself
    let source = "
PROGRAM main
  VAR
    x : REAL;
    nan : REAL;
    eq_result : DINT;
    lt_result : DINT;
    gt_result : DINT;
  END_VAR
  x := 0.0;
  nan := x / 0.0;
  IF nan = nan THEN
    eq_result := 1;
  ELSE
    eq_result := 0;
  END_IF;
  IF nan < 1.0 THEN
    lt_result := 1;
  ELSE
    lt_result := 0;
  END_IF;
  IF nan > 1.0 THEN
    gt_result := 1;
  ELSE
    gt_result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // NaN == NaN is false
    assert_eq!(bufs.vars[2].as_i32(), 0, "NaN == NaN should be false");
    // NaN < 1.0 is false
    assert_eq!(bufs.vars[3].as_i32(), 0, "NaN < 1.0 should be false");
    // NaN > 1.0 is false
    assert_eq!(bufs.vars[4].as_i32(), 0, "NaN > 1.0 should be false");
}

#[test]
fn end_to_end_when_real_nan_ne_then_true() {
    // IEEE 754: NaN <> NaN is true
    let source = "
PROGRAM main
  VAR
    x : REAL;
    nan : REAL;
    result : DINT;
  END_VAR
  x := 0.0;
  nan := x / 0.0;
  IF nan <> nan THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1, "NaN <> NaN should be true");
}

#[test]
fn end_to_end_when_real_inf_arithmetic_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    inf : REAL;
    sum : REAL;
    product : REAL;
  END_VAR
  x := 1.0;
  inf := x / 0.0;
  sum := inf + 1.0;
  product := inf * 2.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let sum = bufs.vars[2].as_f32();
    let product = bufs.vars[3].as_f32();
    assert!(sum.is_infinite() && sum > 0.0, "Inf + 1.0 should be +Inf");
    assert!(
        product.is_infinite() && product > 0.0,
        "Inf * 2.0 should be +Inf"
    );
}

#[test]
fn end_to_end_when_lreal_divide_by_zero_then_inf() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.0;
  y := x / 0.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!(y.is_infinite() && y > 0.0, "expected +Inf, got {y}");
}

#[test]
fn end_to_end_when_lreal_negative_divide_by_zero_then_neg_inf() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := -1.0;
  y := x / 0.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!(y.is_infinite() && y < 0.0, "expected -Inf, got {y}");
}

#[test]
fn end_to_end_when_lreal_zero_divide_by_zero_then_nan() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 0.0;
  y := x / 0.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!(y.is_nan(), "expected NaN, got {y}");
}

#[test]
fn end_to_end_when_lreal_nan_comparison_then_all_false() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    nan : LREAL;
    eq_result : DINT;
    lt_result : DINT;
    gt_result : DINT;
  END_VAR
  x := 0.0;
  nan := x / 0.0;
  IF nan = nan THEN
    eq_result := 1;
  ELSE
    eq_result := 0;
  END_IF;
  IF nan < 1.0 THEN
    lt_result := 1;
  ELSE
    lt_result := 0;
  END_IF;
  IF nan > 1.0 THEN
    gt_result := 1;
  ELSE
    gt_result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 0, "NaN == NaN should be false");
    assert_eq!(bufs.vars[3].as_i32(), 0, "NaN < 1.0 should be false");
    assert_eq!(bufs.vars[4].as_i32(), 0, "NaN > 1.0 should be false");
}

#[test]
fn end_to_end_when_lreal_nan_ne_then_true() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    nan : LREAL;
    result : DINT;
  END_VAR
  x := 0.0;
  nan := x / 0.0;
  IF nan <> nan THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[2].as_i32(), 1, "NaN <> NaN should be true");
}
