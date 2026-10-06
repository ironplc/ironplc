//! End-to-end integration tests for the SQRT function.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

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
    &[("y", 3.0)],
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
    &[("y", 0.0)],
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
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    let y = snapshot.read_as::<f32>("y");
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
    &[("y", std::f64::consts::SQRT_2)],
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
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    let y = snapshot.read_as::<f64>("y");
    assert!(y.is_nan(), "expected NaN, got {y}");
}
