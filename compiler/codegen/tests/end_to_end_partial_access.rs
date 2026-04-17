//! End-to-end tests for byte/word/dword/lword partial access
//! (`.%Bn`, `.%Wn`, `.%Dn`, `.%Ln`).

mod common;
use common::{parse_and_run, try_parse_and_compile};
use ironplc_parser::options::CompilerOptions;

fn opts() -> CompilerOptions {
    CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    }
}

// --- Byte partial access (.%Bn) reads ---

#[test]
fn end_to_end_when_read_byte_0_from_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    d : DWORD;
    r : BYTE;
  END_VAR
  d := DWORD#16#AABBCCDD;
  r := d.%B0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Byte 0 = least significant byte = 0xDD = 221
    assert_eq!(bufs.vars[1].as_i32(), 0xDD);
}

#[test]
fn end_to_end_when_read_byte_3_from_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    d : DWORD;
    r : BYTE;
  END_VAR
  d := DWORD#16#AABBCCDD;
  r := d.%B3;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Byte 3 = most significant byte = 0xAA = 170
    assert_eq!(bufs.vars[1].as_i32(), 0xAA);
}

#[test]
fn end_to_end_when_read_byte_from_lword_then_correct() {
    let source = "
PROGRAM main
  VAR
    l : LWORD;
    r : BYTE;
  END_VAR
  l := LWORD#16#0102030405060708;
  r := l.%B7;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Byte 7 = most significant byte = 0x01
    assert_eq!(bufs.vars[1].as_i32(), 0x01);
}

// --- Word partial access (.%Wn) reads ---

#[test]
fn end_to_end_when_read_word_0_from_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    d : DWORD;
    r : WORD;
  END_VAR
  d := DWORD#16#AABBCCDD;
  r := d.%W0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Word 0 = 0xCCDD
    assert_eq!(bufs.vars[1].as_i32(), 0xCCDD);
}

#[test]
fn end_to_end_when_read_word_1_from_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    d : DWORD;
    r : WORD;
  END_VAR
  d := DWORD#16#AABBCCDD;
  r := d.%W1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Word 1 = 0xAABB
    assert_eq!(bufs.vars[1].as_i32(), 0xAABB);
}

#[test]
fn end_to_end_when_read_word_from_lword_then_correct() {
    let source = "
PROGRAM main
  VAR
    l : LWORD;
    r : WORD;
  END_VAR
  l := LWORD#16#0102030405060708;
  r := l.%W2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Word 2 = bits 32-47 = 0x0304
    assert_eq!(bufs.vars[1].as_i32(), 0x0304);
}

// --- DWord partial access (.%Dn) reads ---

#[test]
fn end_to_end_when_read_dword_1_from_lword_then_correct() {
    let source = "
PROGRAM main
  VAR
    l : LWORD;
    r : DWORD;
  END_VAR
  l := LWORD#16#AABBCCDD11223344;
  r := l.%D1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // DWord 1 = upper 32 bits = 0xAABBCCDD
    assert_eq!(bufs.vars[1].as_i32(), 0xAABBCCDDu32 as i32);
}

// --- Byte partial access (.%Bn) writes ---

#[test]
fn end_to_end_when_write_byte_1_to_dword_then_preserves_others() {
    let source = "
PROGRAM main
  VAR
    d : DWORD;
    r : DWORD;
  END_VAR
  d := DWORD#16#AABBCCDD;
  d.%B1 := BYTE#16#FF;
  r := d;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Byte 1 replaced: 0xAABBCCDD -> 0xAABBFFDD
    assert_eq!(bufs.vars[1].as_i32(), 0xAABBFFDDu32 as i32);
}

#[test]
fn end_to_end_when_write_byte_0_to_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    d : DWORD;
    r : DWORD;
  END_VAR
  d := DWORD#16#AABB0000;
  d.%B0 := BYTE#16#42;
  r := d;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    assert_eq!(bufs.vars[1].as_i32(), 0xAABB0042u32 as i32);
}

// --- Word partial access (.%Wn) writes ---

#[test]
fn end_to_end_when_write_word_to_lword_then_correct() {
    let source = "
PROGRAM main
  VAR
    l : LWORD;
    r : LWORD;
  END_VAR
  l := LWORD#16#0000000000000000;
  l.%W1 := WORD#16#ABCD;
  r := l;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Word 1 = bits 16-31 set to 0xABCD
    assert_eq!(bufs.vars[1].as_i64(), 0x00000000ABCD0000u64 as i64);
}

// --- Partial access on array elements ---

#[test]
fn end_to_end_when_read_byte_from_dword_array_then_correct() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..1] OF DWORD;
    r : BYTE;
  END_VAR
  arr[0] := DWORD#16#AABBCCDD;
  r := arr[0].%B2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Byte 2 = 0xBB
    assert_eq!(bufs.vars[1].as_i32(), 0xBB);
}

#[test]
fn end_to_end_when_write_byte_to_dword_array_then_correct() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..0] OF DWORD;
    r : DWORD;
  END_VAR
  arr[0] := DWORD#16#00000000;
  arr[0].%B3 := BYTE#16#FF;
  r := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    assert_eq!(bufs.vars[1].as_i32(), 0xFF000000u32 as i32);
}

// --- Partial access on struct fields ---

#[test]
fn end_to_end_when_read_byte_from_struct_field_then_correct() {
    let source = "
TYPE MY_STRUCT : STRUCT
    value : DWORD;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    r : BYTE;
  END_VAR
  s.value := DWORD#16#12345678;
  r := s.value.%B1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // Byte 1 = 0x56
    assert_eq!(bufs.vars[1].as_i32(), 0x56);
}

#[test]
fn end_to_end_when_write_byte_to_struct_field_then_correct() {
    let source = "
TYPE MY_STRUCT : STRUCT
    value : DWORD;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    r : DWORD;
  END_VAR
  s.value := DWORD#16#12345678;
  s.value.%B2 := BYTE#16#FF;
  r := s.value;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts());
    // 0x12345678 with byte 2 replaced: 0x12FF5678
    assert_eq!(bufs.vars[1].as_i32(), 0x12FF5678u32 as i32);
}

// --- Compilation gating ---

#[test]
fn end_to_end_when_partial_access_byte_flag_off_then_parse_fails() {
    let source = "
PROGRAM main
  VAR
    d : DWORD;
    r : BYTE;
  END_VAR
  r := d.%B0;
END_PROGRAM
";
    let result = ironplc_parser::parse_program(
        source,
        &ironplc_dsl::core::FileId::default(),
        &CompilerOptions::default(),
    );
    assert!(result.is_err());
}

#[test]
fn end_to_end_when_partial_access_byte_flag_on_then_compiles() {
    let source = "
PROGRAM main
  VAR
    d : DWORD;
    r : BYTE;
  END_VAR
  r := d.%B0;
END_PROGRAM
";
    let result = try_parse_and_compile(source, &opts());
    assert!(
        result.is_ok(),
        "expected compile to succeed, got error: {:?}",
        result.err()
    );
}
