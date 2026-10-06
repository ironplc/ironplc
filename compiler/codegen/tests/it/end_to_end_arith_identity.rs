//! End-to-end integration tests for the arithmetic-identity peephole.
//!
//! Every case runs `x := <x>; y := <expr>; z := 1.0 / y;` (or the integer
//! equivalent without `z`) and checks the result with the constant on either
//! side of the operator. Float results are compared by bit pattern, because
//! `-0.0 == 0.0` and the sign of zero is the whole point: IEEE 754 says
//! `(-0.0) + 0.0 = +0.0`, so the optimizer must not remove the add, while
//! `(-0.0) - 0.0 = -0.0`, so it may remove the subtract. `z` shows the
//! consequence a program can observe: `1.0 / (+0.0)` is `+inf` and
//! `1.0 / (-0.0)` is `-inf`.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::Snapshot;

/// A floating-point width.
#[derive(Clone, Copy)]
enum Float {
    Real,
    Lreal,
}

impl Float {
    fn decl(self) -> &'static str {
        match self {
            Float::Real => "REAL",
            Float::Lreal => "LREAL",
        }
    }
}

/// Every expected value is exact in f32 and f64, so one case list serves
/// both widths.
#[rstest]
#[case::neg_zero_plus_zero("-0.0", "x + 0.0", 0.0, f64::INFINITY)]
#[case::zero_plus_neg_zero("-0.0", "0.0 + x", 0.0, f64::INFINITY)]
#[case::neg_zero_minus_zero("-0.0", "x - 0.0", -0.0, f64::NEG_INFINITY)]
#[case::zero_minus_neg_zero("-0.0", "0.0 - x", 0.0, f64::INFINITY)]
#[case::value_plus_zero("4.0", "x + 0.0", 4.0, 0.25)]
#[case::zero_plus_value("4.0", "0.0 + x", 4.0, 0.25)]
#[case::value_minus_zero("4.0", "x - 0.0", 4.0, 0.25)]
#[case::zero_minus_value("4.0", "0.0 - x", -4.0, -0.25)]
#[case::value_times_one("4.0", "x * 1.0", 4.0, 0.25)]
#[case::one_times_value("4.0", "1.0 * x", 4.0, 0.25)]
#[case::value_over_one("4.0", "x / 1.0", 4.0, 0.25)]
#[case::one_over_value("4.0", "1.0 / x", 0.25, 4.0)]
fn end_to_end_when_float_identity_then_value_and_sign_correct(
    #[values(Float::Real, Float::Lreal)] float: Float,
    #[case] x: &str,
    #[case] expr: &str,
    #[case] y: f64,
    #[case] z: f64,
) {
    let ty = float.decl();
    let source = format!(
        "
PROGRAM main
  VAR
    x : {ty};
    y : {ty};
    z : {ty};
  END_VAR
  x := {x};
  y := {expr};
  z := 1.0 / y;
END_PROGRAM
"
    );
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());

    // A REAL reads widened exactly, so both widths compare as f64.
    assert_eq!(
        snapshot.read_as::<f64>("y").to_bits(),
        y.to_bits(),
        "y mismatch"
    );
    assert_eq!(
        snapshot.read_as::<f64>("z").to_bits(),
        z.to_bits(),
        "z mismatch"
    );
}

#[rstest]
#[case::value_plus_zero("x + 0", -7)]
#[case::zero_plus_value("0 + x", -7)]
#[case::value_minus_zero("x - 0", -7)]
#[case::zero_minus_value("0 - x", 7)]
#[case::value_times_one("x * 1", -7)]
#[case::one_times_value("1 * x", -7)]
#[case::value_over_one("x / 1", -7)]
#[case::one_over_value("1 / x", 0)]
fn end_to_end_when_int_identity_then_value_correct(
    #[values("DINT", "LINT")] ty: &str,
    #[case] expr: &str,
    #[case] y: i64,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : {ty};
    y : {ty};
  END_VAR
  x := -7;
  y := {expr};
END_PROGRAM
"
    );
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());

    assert_eq!(snapshot.read_as::<i64>("y"), y);
}
