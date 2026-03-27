//! End-to-end integration tests for bit access on integer variables (e.g., `a.0`).

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

// --- Bit read tests ---

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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 0x8000 bit 15 is 1 → TRUE
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_read_bit_of_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : BOOL;
  END_VAR
  x := 65536;
  y := x.16;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 65536 = 0x10000, bit 16 = 1
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_read_bit_of_int_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : BOOL;
  END_VAR
  x := 10;
  y := x.1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 10 = 0b1010, bit 1 = 1
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_function_with_dint_bit_access_then_correct() {
    let source = "
FUNCTION FOO : INT
  VAR_INPUT
    A : DINT;
  END_VAR
  IF A.0 THEN
    FOO := 1;
  END_IF;
END_FUNCTION

PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := FOO(A := 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 5 = 0b101, bit 0 is 1 → TRUE, so FOO returns 1
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

// --- Bit write tests ---

#[test]
fn end_to_end_when_write_bit_0_set_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 0;
  x.0 := TRUE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Set bit 0: 0 -> 1
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_write_bit_3_set_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 0;
  x.3 := TRUE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Set bit 3: 0 -> 8
    assert_eq!(bufs.vars[0].as_i32(), 8);
}

#[test]
fn end_to_end_when_write_bit_0_clear_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 255;
  x.0 := FALSE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Clear bit 0: 255 -> 254
    assert_eq!(bufs.vars[0].as_i32(), 254);
}

#[test]
fn end_to_end_when_write_bit_7_clear_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 255;
  x.7 := FALSE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Clear bit 7: 255 -> 127
    assert_eq!(bufs.vars[0].as_i32(), 127);
}

#[test]
fn end_to_end_when_write_bit_preserves_other_bits_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 170;
  x.0 := TRUE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 170 = 0b10101010, set bit 0 -> 0b10101011 = 171
    assert_eq!(bufs.vars[0].as_i32(), 171);
}

#[test]
fn end_to_end_when_write_bit_of_word_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
  END_VAR
  x := 0;
  x.8 := TRUE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Set bit 8: 0 -> 256
    assert_eq!(bufs.vars[0].as_i32(), 256);
}

#[test]
fn end_to_end_when_write_bit_of_dint_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 0;
  x.16 := TRUE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Set bit 16: 0 -> 65536
    assert_eq!(bufs.vars[0].as_i32(), 65536);
}

// --- Multiple bit operations ---

#[test]
fn end_to_end_when_set_multiple_bits_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 0;
  x.0 := TRUE;
  x.2 := TRUE;
  x.4 := TRUE;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Set bits 0, 2, 4: 0b00010101 = 21
    assert_eq!(bufs.vars[0].as_i32(), 21);
}

#[test]
fn end_to_end_when_read_after_write_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BOOL;
  END_VAR
  x := 0;
  x.3 := TRUE;
  y := x.3;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 8); // x = 8
    assert_eq!(bufs.vars[1].as_i32(), 1); // y = TRUE
}
