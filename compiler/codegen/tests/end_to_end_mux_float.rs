//! End-to-end integration tests for the MUX function with float types.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_mux_real_k0_then_returns_in0() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := MUX(0, 10.5, 20.5, 30.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[0].as_f32();
    assert!((y - 10.5).abs() < 1e-5, "expected 10.5, got {y}");
}

#[test]
fn end_to_end_when_mux_real_k2_then_returns_in2() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := MUX(2, 10.5, 20.5, 30.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[0].as_f32();
    assert!((y - 30.5).abs() < 1e-5, "expected 30.5, got {y}");
}

#[test]
fn end_to_end_when_mux_real_k_out_of_range_then_clamps_to_last() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := MUX(5, 10.5, 20.5, 30.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[0].as_f32();
    assert!((y - 30.5).abs() < 1e-5, "expected 30.5, got {y}");
}

#[test]
fn end_to_end_when_mux_lreal_k0_then_returns_in0() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := MUX(0, 10.5, 20.5, 30.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[0].as_f64();
    assert!((y - 10.5).abs() < 1e-12, "expected 10.5, got {y}");
}

#[test]
fn end_to_end_when_mux_lreal_k1_then_returns_in1() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := MUX(1, 10.5, 20.5, 30.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[0].as_f64();
    assert!((y - 20.5).abs() < 1e-12, "expected 20.5, got {y}");
}

#[test]
fn end_to_end_when_mux_lreal_k_out_of_range_then_clamps_to_last() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := MUX(5, 10.5, 20.5, 30.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[0].as_f64();
    assert!((y - 30.5).abs() < 1e-12, "expected 30.5, got {y}");
}
