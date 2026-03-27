//! End-to-end integration tests for bit shift/rotate functions (SHL, SHR, ROL, ROR).

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

// --- SHL ---

#[test]
fn end_to_end_when_shl_byte_then_shifts_left() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#0F;
  y := SHL(x, 4);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0x0F);
    assert_eq!(bufs.vars[1].as_i32(), 0xF0);
}

#[test]
fn end_to_end_when_shl_dword_then_shifts_left() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#0000_000F;
  y := SHL(x, 16);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0x0F);
    assert_eq!(bufs.vars[1].as_i32(), 0x000F_0000_u32 as i32);
}

#[test]
fn end_to_end_when_shl_lword_then_shifts_left_64bit() {
    let source = "
PROGRAM main
  VAR
    x : LWORD;
    y : LWORD;
  END_VAR
  x := LWORD#16#01;
  y := SHL(x, 32);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i64(), 0x01);
    assert_eq!(bufs.vars[1].as_i64(), 0x1_0000_0000);
}

// --- SHR ---

#[test]
fn end_to_end_when_shr_word_then_shifts_right() {
    let source = "
PROGRAM main
  VAR
    x : WORD;
    y : WORD;
  END_VAR
  x := WORD#16#FF00;
  y := SHR(x, 8);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0xFF00);
    assert_eq!(bufs.vars[1].as_i32(), 0x00FF);
}

#[test]
fn end_to_end_when_shr_byte_then_shifts_right() {
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#F0;
  y := SHR(x, 4);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0xF0);
    assert_eq!(bufs.vars[1].as_i32(), 0x0F);
}

// --- ROL ---

#[test]
fn end_to_end_when_rol_byte_then_rotates_within_8_bits() {
    // ROL(BYTE#16#81, 1) should give 0x03 (bit 7 wraps to bit 0 within 8 bits)
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#81;
  y := ROL(x, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0x81);
    assert_eq!(bufs.vars[1].as_i32(), 0x03);
}

#[test]
fn end_to_end_when_rol_word_then_rotates_within_16_bits() {
    // ROL(WORD#16#8001, 1) = 0x0003
    let source = "
PROGRAM main
  VAR
    x : WORD;
    y : WORD;
  END_VAR
  x := WORD#16#8001;
  y := ROL(x, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0x8001);
    assert_eq!(bufs.vars[1].as_i32(), 0x0003);
}

#[test]
fn end_to_end_when_rol_dword_then_rotates_left() {
    // ROL(DWORD#16#80000001, 1) = 0x00000003
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#80000001;
  y := ROL(x, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0x80000001_u32 as i32);
    assert_eq!(bufs.vars[1].as_i32(), 0x00000003);
}

// --- ROR ---

#[test]
fn end_to_end_when_ror_dword_then_rotates_right() {
    // ROR(DWORD#16#00000001, 1) = 0x80000000 (bit 0 wraps to bit 31)
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#00000001;
  y := ROR(x, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0x01);
    assert_eq!(bufs.vars[1].as_i32(), 0x80000000_u32 as i32);
}

#[test]
fn end_to_end_when_ror_byte_then_rotates_within_8_bits() {
    // ROR(BYTE#16#01, 1) = 0x80 (bit 0 wraps to bit 7 within 8 bits)
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#01;
  y := ROR(x, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0x01);
    assert_eq!(bufs.vars[1].as_i32(), 0x80);
}

#[test]
fn end_to_end_when_shl_byte_overflow_then_truncates() {
    // SHL(BYTE#16#FF, 4) = 0xF0 (shifted to 0xFF0, truncated to u8 = 0xF0)
    let source = "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#FF;
  y := SHL(x, 4);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 0xFF);
    assert_eq!(bufs.vars[1].as_i32(), 0xF0);
}

#[test]
fn end_to_end_when_shl_with_zero_shift_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#DEADBEEF;
  y := SHL(x, 0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), bufs.vars[0].as_i32());
}

// --- Nested function calls ---

#[test]
fn end_to_end_when_shr_with_abs_then_computes_correctly() {
    let source = "
PROGRAM main
  VAR
    a : DINT;
    result : DINT;
  END_VAR
  a := -8;
  result := SHR(ABS(a), 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), -8);
    assert_eq!(bufs.vars[1].as_i32(), 4);
}
