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

// --- Bitwise AND ---

#[test]
fn end_to_end_when_byte_and_then_bitwise() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#FF;
  y := x AND BYTE#16#0F;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 0xFF);
    assert_eq!(bufs.vars[1].as_i32(), 0x0F);
}

// --- Bitwise OR ---

#[test]
fn end_to_end_when_byte_or_then_bitwise() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#F0;
  y := x OR BYTE#16#0F;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 0xF0);
    assert_eq!(bufs.vars[1].as_i32(), 0xFF);
}

// --- Bitwise XOR ---

#[test]
fn end_to_end_when_byte_xor_then_bitwise() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#FF;
  y := x XOR BYTE#16#0F;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 0xFF);
    assert_eq!(bufs.vars[1].as_i32(), 0xF0);
}

// --- Bitwise NOT ---

#[test]
fn end_to_end_when_byte_not_then_truncated() {
    // NOT BYTE#16#0F should be BYTE#16#F0 (= 240), not 0xFFFFFFF0
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#0F;
  y := NOT x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 0x0F);
    assert_eq!(bufs.vars[1].as_i32(), 0xF0);
}

// --- DWORD bitwise ops (full 32-bit) ---

#[test]
fn end_to_end_when_dword_and_then_bitwise() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#FFFF0000;
  y := x AND DWORD#16#FF00FF00;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 0xFFFF0000 AND 0xFF00FF00 = 0xFF000000
    assert_eq!(bufs.vars[1].as_i32() as u32, 0xFF00_0000);
}

#[test]
fn end_to_end_when_dword_not_then_bitwise() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := 0;
  y := NOT x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // NOT 0 = 0xFFFFFFFF (as i32: -1)
    assert_eq!(bufs.vars[1].as_i32(), -1);
}

// --- LWORD bitwise ops (64-bit) ---

#[test]
fn end_to_end_when_lword_and_then_bitwise() {
    let source = "
PROGRAM main
  VAR
    x : LWORD;
    y : LWORD;
  END_VAR
  x := LWORD#16#FF;
  y := x AND LWORD#16#0F;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 0x0F);
}

#[test]
fn end_to_end_when_lword_not_then_bitwise() {
    let source = "
PROGRAM main
  VAR
    x : LWORD;
    y : LWORD;
  END_VAR
  x := 0;
  y := NOT x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // NOT 0_i64 = -1_i64
    assert_eq!(bufs.vars[1].as_i64(), -1);
}

// --- NOT in IF condition (inline truncation correctness) ---

#[test]
fn end_to_end_when_byte_not_in_if_then_correct() {
    // NOT BYTE#16#FF = 0 (after truncation), so IF body should NOT execute
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    result : BYTE;
  END_VAR
  x := BYTE#16#FF;
  result := 0;
  IF NOT x THEN
    result := 1;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // NOT 0xFF → BIT_NOT → 0xFFFFFF00 → TRUNC_U8 → 0x00 → IF sees 0 → skip body
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_byte_not_zero_in_if_then_enters_body() {
    // NOT BYTE#0 = 0xFF (after truncation), so IF body SHOULD execute
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    result : BYTE;
  END_VAR
  x := 0;
  result := 0;
  IF NOT x THEN
    result := 1;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // NOT 0x00 → BIT_NOT → 0xFFFFFFFF → TRUNC_U8 → 0xFF → IF sees non-zero → enter body
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

// --- WORD NOT with truncation ---

#[test]
fn end_to_end_when_word_not_then_truncated() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
    y : WORD;
  END_VAR
  x := WORD#16#FF00;
  y := NOT x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // NOT 0xFF00 at 32-bit = 0xFFFF00FF, truncated to u16 = 0x00FF
    assert_eq!(bufs.vars[1].as_i32(), 0x00FF);
}
