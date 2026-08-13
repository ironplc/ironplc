//! End-to-end integration tests for the SQRT function.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

e2e_f32_near!(
    end_to_end_when_sqrt_real_perfect_square_then_correct,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 9.0;
  y := SQRT(x);
END_PROGRAM
",
    &[(1, 3.0)],
);

e2e_f32_near!(
    end_to_end_when_sqrt_real_zero_then_zero,
    1e-5,
    "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := SQRT(x);
END_PROGRAM
",
    &[(1, 0.0)],
);

#[test]
fn end_to_end_when_sqrt_real_negative_then_nan() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := -1.0;
  y := SQRT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!(y.is_nan(), "expected NaN, got {y}");
}

e2e_f64_near!(
    end_to_end_when_sqrt_lreal_then_correct,
    1e-12,
    "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 2.0;
  y := SQRT(x);
END_PROGRAM
",
    &[(1, std::f64::consts::SQRT_2)],
);

#[test]
fn end_to_end_when_sqrt_lreal_negative_then_nan() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := -1.0;
  y := SQRT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!(y.is_nan(), "expected NaN, got {y}");
}
