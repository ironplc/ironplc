//! End-to-end integration tests for bit access on integer variables (e.g., `a.0`).

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_dint_bit_access_0_on_odd_then_true() {
    let source = "
PROGRAM main
  VAR
    a : DINT;
    result : BOOL;
  END_VAR
  a := 5;
  result := a.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 5 = 0b101, bit 0 is 1 → TRUE
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_dint_bit_access_0_on_even_then_false() {
    let source = "
PROGRAM main
  VAR
    a : DINT;
    result : BOOL;
  END_VAR
  a := 4;
  result := a.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 4 = 0b100, bit 0 is 0 → FALSE
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_dint_bit_access_2_then_correct() {
    let source = "
PROGRAM main
  VAR
    a : DINT;
    result : BOOL;
  END_VAR
  a := 5;
  result := a.2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 5 = 0b101, bit 2 is 1 → TRUE
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_dint_bit_access_1_then_false() {
    let source = "
PROGRAM main
  VAR
    a : DINT;
    result : BOOL;
  END_VAR
  a := 5;
  result := a.1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 5 = 0b101, bit 1 is 0 → FALSE
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_byte_bit_access_7_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    result : BOOL;
  END_VAR
  x := BYTE#16#80;
  result := x.7;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 0x80 = 0b10000000, bit 7 is 1 → TRUE
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_word_bit_access_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
    result : BOOL;
  END_VAR
  x := WORD#16#8000;
  result := x.15;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 0x8000 bit 15 is 1 → TRUE
    assert_eq!(bufs.vars[1].as_i32(), 1);
}
