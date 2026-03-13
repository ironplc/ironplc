//! End-to-end integration tests for bit access code generation.
//!
//! Tests reading and writing individual bits of integer and bit string variables.

mod common;

use common::parse_and_run;

// --- Bit read tests ---

#[test]
fn end_to_end_when_read_bit_0_of_byte_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BOOL;
  END_VAR
  x := 5;
  y := x.0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 5 = 0b101, bit 0 = 1
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_read_bit_1_of_byte_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BOOL;
  END_VAR
  x := 5;
  y := x.1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 5 = 0b101, bit 1 = 0
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_read_bit_2_of_byte_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BOOL;
  END_VAR
  x := 5;
  y := x.2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 5 = 0b101, bit 2 = 1
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_read_bit_7_of_byte_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BOOL;
  END_VAR
  x := 128;
  y := x.7;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 128 = 0b10000000, bit 7 = 1
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_read_bit_of_word_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
    y : BOOL;
  END_VAR
  x := 256;
  y := x.8;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 256 = 0x100, bit 8 = 1
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
    // 10 = 0b1010, bit 1 = 1
    assert_eq!(bufs.vars[1].as_i32(), 1);
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
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
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 8); // x = 8
    assert_eq!(bufs.vars[1].as_i32(), 1); // y = TRUE
}
