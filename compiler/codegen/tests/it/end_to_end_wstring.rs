//! End-to-end integration tests for WSTRING (UTF-16LE / 2-byte code units).
//!
//! Mirrors `end_to_end_string.rs` but exercises the wide-string path: each
//! variable's data-region header records `char_width = 2`, payload bytes are
//! UTF-16LE little-endian code units, and length fields stay in code units
//! rather than bytes.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;
use ironplc_container::STRING_HEADER_BYTES;

/// Reads the max_length field of a WSTRING variable.
fn read_max_length(data_region: &[u8], data_offset: usize) -> u16 {
    u16::from_le_bytes([data_region[data_offset], data_region[data_offset + 1]])
}

/// Reads the cur_length field (code units, not bytes).
fn read_cur_length(data_region: &[u8], data_offset: usize) -> u16 {
    u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]])
}

/// Reads the char_width field (1 for STRING, 2 for WSTRING).
fn read_char_width(data_region: &[u8], data_offset: usize) -> u16 {
    u16::from_le_bytes([data_region[data_offset + 4], data_region[data_offset + 5]])
}

/// Decodes a WSTRING value (UTF-16LE BMP code units) into a Rust String.
/// Surrogate-pair-aware iteration is out of scope per ADR-0016, so we treat
/// each code unit as a Unicode code point.
fn read_wstring(data_region: &[u8], data_offset: usize) -> String {
    let cur_len = read_cur_length(data_region, data_offset) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let mut s = String::with_capacity(cur_len);
    for i in 0..cur_len {
        let lo = data_region[data_start + i * 2];
        let hi = data_region[data_start + i * 2 + 1];
        let cu = u16::from_le_bytes([lo, hi]) as u32;
        if let Some(c) = char::from_u32(cu) {
            s.push(c);
        }
    }
    s
}

#[test]
fn end_to_end_when_wstring_initial_value_then_variable_initialized() {
    let source = "
PROGRAM main
  VAR
    x : WSTRING := \"hello\";
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_wstring(&bufs.data_region, 0), "hello");
    assert_eq!(read_max_length(&bufs.data_region, 0), 254);
    assert_eq!(read_char_width(&bufs.data_region, 0), 2);
}

#[test]
fn end_to_end_when_wstring_with_explicit_length_then_max_length_set() {
    let source = "
PROGRAM main
  VAR
    x : WSTRING[10] := \"hi\";
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_wstring(&bufs.data_region, 0), "hi");
    assert_eq!(read_max_length(&bufs.data_region, 0), 10);
    assert_eq!(read_char_width(&bufs.data_region, 0), 2);
    // Code units, not bytes.
    assert_eq!(read_cur_length(&bufs.data_region, 0), 2);
}

#[test]
fn end_to_end_when_wstring_no_initial_value_then_empty() {
    let source = "
PROGRAM main
  VAR
    x : WSTRING;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_wstring(&bufs.data_region, 0), "");
    assert_eq!(read_cur_length(&bufs.data_region, 0), 0);
    assert_eq!(read_char_width(&bufs.data_region, 0), 2);
}

#[test]
fn end_to_end_when_wstring_payload_uses_utf16le_encoding() {
    // The literal "hi" should be encoded as h=0x68, i=0x69 followed by zero
    // high bytes (UTF-16LE, BMP only).
    let source = "
PROGRAM main
  VAR
    x : WSTRING[10] := \"hi\";
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let data_start = STRING_HEADER_BYTES;
    assert_eq!(bufs.data_region[data_start], b'h');
    assert_eq!(bufs.data_region[data_start + 1], 0);
    assert_eq!(bufs.data_region[data_start + 2], b'i');
    assert_eq!(bufs.data_region[data_start + 3], 0);
}

#[test]
fn end_to_end_when_string_and_wstring_coexist_then_independent() {
    // Mixed STRING and WSTRING in the same program: each gets its own
    // char_width tag and there is no interference between the two storage
    // regions.
    let source = "
PROGRAM main
  VAR
    s : STRING[10] := 'foo';
    w : WSTRING[10] := \"bar\";
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // STRING at offset 0 (char_width = 1, max_len = 10 -> stride = 6 + 10 = 16).
    assert_eq!(read_char_width(&bufs.data_region, 0), 1);
    assert_eq!(read_cur_length(&bufs.data_region, 0), 3);
    let s_start = STRING_HEADER_BYTES;
    assert_eq!(&bufs.data_region[s_start..s_start + 3], b"foo");

    // WSTRING at offset 16 (char_width = 2, max_len = 10 -> stride = 6 + 20 = 26).
    let w_offset = STRING_HEADER_BYTES + 10;
    assert_eq!(read_char_width(&bufs.data_region, w_offset), 2);
    assert_eq!(read_wstring(&bufs.data_region, w_offset), "bar");
}

#[test]
fn end_to_end_when_wstring_assignment_then_value_copied() {
    let source = "
PROGRAM main
  VAR
    src : WSTRING[10] := \"abc\";
    dst : WSTRING[10];
  END_VAR
  dst := src;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let dst_offset = STRING_HEADER_BYTES + 10 * 2;
    assert_eq!(read_char_width(&bufs.data_region, dst_offset), 2);
    assert_eq!(read_wstring(&bufs.data_region, dst_offset), "abc");
}

#[test]
fn end_to_end_when_len_of_wstring_then_returns_code_unit_count() {
    // LEN must report the count in code units (characters), not bytes.
    let source = "
PROGRAM main
  VAR
    w : WSTRING[20] := \"hello\";
    n : DINT;
  END_VAR
  n := LEN(w);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // var[0] holds w's data_offset; var[1] holds n.
    assert_eq!(bufs.vars[1].as_i32(), 5);
}

#[test]
fn end_to_end_when_concat_two_wstrings_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    a : WSTRING[10] := \"foo\";
    b : WSTRING[10] := \"bar\";
    c : WSTRING[20];
  END_VAR
  c := CONCAT(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // a at offset 0, b at offset STRING_HEADER_BYTES + 10*2, c at next slot.
    let a_stride = STRING_HEADER_BYTES + 10 * 2;
    let b_stride = STRING_HEADER_BYTES + 10 * 2;
    let c_offset = a_stride + b_stride;
    assert_eq!(read_char_width(&bufs.data_region, c_offset), 2);
    assert_eq!(read_wstring(&bufs.data_region, c_offset), "foobar");
}
