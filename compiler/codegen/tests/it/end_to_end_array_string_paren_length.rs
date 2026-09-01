//! End-to-end integration tests for `ARRAY[..] OF STRING(n)` — the
//! parenthesis length delimiter used as an array element type.
//!
//! The parenthesis form is a vendor extension gated behind
//! `allow_paren_string_length` (see
//! `parser/src/rule_token_no_paren_string_length.rs`). These tests pin that
//! the element-type position produces the same layout the bracket form does:
//! a length that was parsed but dropped would silently fall back to the
//! default 254, so each case runs the program and reads the stride back.

use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

/// Reads the `max_length` header field (code units).
fn read_max_length(data_region: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data_region[offset], data_region[offset + 1]])
}

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

/// Reads a WSTRING value (UTF-16LE code units) from the data region.
fn read_wstring(data_region: &[u8], offset: usize) -> String {
    let cur_len = u16::from_le_bytes([data_region[offset + 2], data_region[offset + 3]]) as usize;
    let data_start = offset + STRING_HEADER_BYTES;
    let units: Vec<u16> = (0..cur_len)
        .map(|i| {
            let b = data_start + i * 2;
            u16::from_le_bytes([data_region[b], data_region[b + 1]])
        })
        .collect();
    String::from_utf16(&units).unwrap()
}

fn paren_string_length_options() -> CompilerOptions {
    CompilerOptions {
        allow_paren_string_length: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn array_of_string_paren_length_when_assign_then_stores_value() {
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..3] OF STRING(10);
  END_VAR
  names[1] := 'hello';
  names[2] := 'world';
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &paren_string_length_options());

    let base_offset = bufs.vars[0].as_i32() as usize;
    // The stride proves the length came from the parentheses: a dropped
    // length would leave the default 254 here.
    let stride = STRING_HEADER_BYTES + 10;

    assert_eq!(read_max_length(&bufs.data_region, base_offset), 10);
    assert_eq!(read_string(&bufs.data_region, base_offset), "hello");
    assert_eq!(
        read_string(&bufs.data_region, base_offset + stride),
        "world"
    );
    assert_eq!(read_string(&bufs.data_region, base_offset + 2 * stride), "");
}

#[test]
fn array_of_wstring_paren_length_when_assign_then_stores_value() {
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..2] OF WSTRING(8);
  END_VAR
  names[1] := \"hi\";
  names[2] := \"there\";
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &paren_string_length_options());

    let base_offset = bufs.vars[0].as_i32() as usize;
    // WSTRING stores two bytes per code unit (ADR-0016).
    let stride = STRING_HEADER_BYTES + 8 * 2;

    assert_eq!(read_max_length(&bufs.data_region, base_offset), 8);
    assert_eq!(read_wstring(&bufs.data_region, base_offset), "hi");
    assert_eq!(
        read_wstring(&bufs.data_region, base_offset + stride),
        "there"
    );
}

#[test]
fn array_of_string_paren_length_when_truncated_then_respects_max_length() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..2] OF STRING(3);
  END_VAR
  arr[1] := 'abcdefgh';
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &paren_string_length_options());

    let base_offset = bufs.vars[0].as_i32() as usize;
    // Truncation to 3 characters is only possible if the parenthesised
    // length reached the layout.
    assert_eq!(read_string(&bufs.data_region, base_offset), "abc");
}

#[test]
fn array_of_string_paren_length_when_initial_values_then_populated() {
    let source = "
PROGRAM main
  VAR
    days : ARRAY[1..3] OF STRING(10) := ['Mon', 'Tue', 'Wed'];
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &paren_string_length_options());

    let base_offset = bufs.vars[0].as_i32() as usize;
    let stride = STRING_HEADER_BYTES + 10;

    assert_eq!(read_string(&bufs.data_region, base_offset), "Mon");
    assert_eq!(read_string(&bufs.data_region, base_offset + stride), "Tue");
    assert_eq!(
        read_string(&bufs.data_region, base_offset + 2 * stride),
        "Wed"
    );
}
