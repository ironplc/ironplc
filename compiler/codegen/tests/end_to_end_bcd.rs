//! End-to-end integration tests for the BCD_TO_INT and INT_TO_BCD functions.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_bcd_to_int_byte_then_decodes() {
    let source = "
PROGRAM main
  VAR
    bcd_val : BYTE;
    result : USINT;
  END_VAR
  bcd_val := BYTE#16#42;
  result := BCD_TO_INT(bcd_val);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result = bufs.vars[1].as_i32() as u8;
    assert_eq!(result, 42, "expected 42, got {result}");
}

#[test]
fn end_to_end_when_bcd_to_int_word_then_decodes() {
    let source = "
PROGRAM main
  VAR
    bcd_val : WORD;
    result : UINT;
  END_VAR
  bcd_val := WORD#16#1234;
  result := BCD_TO_INT(bcd_val);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result = bufs.vars[1].as_i32() as u16;
    assert_eq!(result, 1234, "expected 1234, got {result}");
}

#[test]
fn end_to_end_when_bcd_to_int_byte_zero_then_zero() {
    let source = "
PROGRAM main
  VAR
    bcd_val : BYTE;
    result : USINT;
  END_VAR
  bcd_val := BYTE#16#00;
  result := BCD_TO_INT(bcd_val);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result = bufs.vars[1].as_i32() as u8;
    assert_eq!(result, 0, "expected 0, got {result}");
}

#[test]
fn end_to_end_when_int_to_bcd_usint_then_encodes() {
    let source = "
PROGRAM main
  VAR
    int_val : USINT;
    result : BYTE;
  END_VAR
  int_val := USINT#42;
  result := INT_TO_BCD(int_val);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result = bufs.vars[1].as_i32() as u8;
    assert_eq!(result, 0x42, "expected 0x42, got 0x{result:02X}");
}

#[test]
fn end_to_end_when_int_to_bcd_uint_then_encodes() {
    let source = "
PROGRAM main
  VAR
    int_val : UINT;
    result : WORD;
  END_VAR
  int_val := UINT#1234;
  result := INT_TO_BCD(int_val);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result = bufs.vars[1].as_i32() as u16;
    assert_eq!(result, 0x1234, "expected 0x1234, got 0x{result:04X}");
}

#[test]
fn end_to_end_when_bcd_to_int_roundtrip_then_matches() {
    let source = "
PROGRAM main
  VAR
    original : USINT;
    bcd_val : BYTE;
    result : USINT;
  END_VAR
  original := USINT#73;
  bcd_val := INT_TO_BCD(original);
  result := BCD_TO_INT(bcd_val);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result = bufs.vars[2].as_i32() as u8;
    assert_eq!(result, 73, "expected 73, got {result}");
}
