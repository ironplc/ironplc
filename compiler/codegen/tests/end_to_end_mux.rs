//! End-to-end integration tests for the MUX function.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_mux_k0_then_returns_in0() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(0, 10, 20, 30);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 10);
}

#[test]
fn end_to_end_when_mux_k1_then_returns_in1() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(1, 10, 20, 30);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 20);
}

#[test]
fn end_to_end_when_mux_k2_then_returns_in2() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(2, 10, 20, 30);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 30);
}

#[test]
fn end_to_end_when_mux_k_out_of_range_then_clamps_to_last() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(5, 10, 20, 30);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // K=5 is out of range (only 3 inputs), clamps to last = 30
    assert_eq!(bufs.vars[0].as_i32(), 30);
}

#[test]
fn end_to_end_when_mux_k_negative_then_clamps_to_first() {
    let source = "
PROGRAM main
  VAR
    k : DINT;
    y : DINT;
  END_VAR
  k := -1;
  y := MUX(k, 10, 20, 30);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // K=-1 clamps to 0 = first input = 10
    assert_eq!(bufs.vars[1].as_i32(), 10);
}

#[test]
fn end_to_end_when_mux_with_variable_selector_then_selects() {
    let source = "
PROGRAM main
  VAR
    k : DINT;
    y : DINT;
  END_VAR
  k := 1;
  y := MUX(k, 100, 200, 300);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 200);
}

#[test]
fn end_to_end_when_mux_2_inputs_then_works() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(1, 42, 99);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 99);
}

#[test]
fn end_to_end_when_mux_4_inputs_then_works() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(3, 10, 20, 30, 40);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 40);
}

#[test]
fn end_to_end_when_mux_16_inputs_then_selects_last() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(15, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 16);
}

#[test]
fn end_to_end_when_mux_16_inputs_k0_then_selects_first() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_mux_16_inputs_k7_then_selects_middle() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(7, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 8);
}

#[test]
fn end_to_end_when_mux_k_equals_input_count_then_clamps_to_last() {
    // K=3 with 3 inputs (indices 0..2), should clamp to IN2
    let source = "
PROGRAM main
  VAR
    y : DINT;
  END_VAR
  y := MUX(3, 10, 20, 30);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 30);
}
