//! End-to-end integration tests for bit string types (BYTE, WORD, DWORD, LWORD).

mod common;

use common::parse_and_run;

// --- BYTE (8-bit unsigned, 0..255) ---

#[test]
fn end_to_end_when_byte_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 200;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 200);
}

#[test]
fn end_to_end_when_byte_overflow_then_wraps() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 255 + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 256 truncated to u8 wraps to 0
    assert_eq!(bufs.vars[0].as_i32(), 0);
}

#[test]
fn end_to_end_when_byte_arithmetic_then_truncates() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := 200;
  y := x + 100;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 200 + 100 = 300, truncated to u8 = 44
    assert_eq!(bufs.vars[0].as_i32(), 200);
    assert_eq!(bufs.vars[1].as_i32(), 44);
}

// --- WORD (16-bit unsigned, 0..65535) ---

#[test]
fn end_to_end_when_word_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
  END_VAR
  x := 50000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 50000);
}

#[test]
fn end_to_end_when_word_overflow_then_wraps() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
  END_VAR
  x := 65535 + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 65536 truncated to u16 wraps to 0
    assert_eq!(bufs.vars[0].as_i32(), 0);
}

// --- DWORD (32-bit unsigned, 0..4294967295) ---

#[test]
fn end_to_end_when_dword_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
  END_VAR
  x := 1000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 1000);
}

#[test]
fn end_to_end_when_dword_comparison_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    result : DWORD;
  END_VAR
  x := 100;
  IF x > 50 THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

// --- LWORD (64-bit unsigned, 0..2^64-1) ---

#[test]
fn end_to_end_when_lword_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LWORD;
  END_VAR
  x := 100000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 100000);
}

#[test]
fn end_to_end_when_lword_comparison_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LWORD;
    y : LWORD;
  END_VAR
  x := 500;
  IF x > 100 THEN
    y := 1;
  ELSE
    y := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 1);
}

// --- Bit string literals with base prefixes ---

#[test]
fn end_to_end_when_hex_bit_string_literal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
  END_VAR
  x := DWORD#16#FF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 255);
}

#[test]
fn end_to_end_when_binary_bit_string_literal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := BYTE#2#11111111;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 255);
}

#[test]
fn end_to_end_when_octal_bit_string_literal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
  END_VAR
  x := WORD#8#377;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 255);
}
