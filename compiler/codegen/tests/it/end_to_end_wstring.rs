//! End-to-end integration tests for WSTRING (UTF-16LE) support.
//!
//! These compile and execute real `.st` programs through the full pipeline
//! (parse → analyze → codegen → VM) and inspect the resulting data region.
//! WSTRING stores two bytes per code unit (UTF-16LE per ADR-0016); a string of
//! `n` code units occupies `n * 2` data bytes after the 6-byte header.

use ironplc_container::debug_section::iec_type_tag;
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::CompilerOptions;

use crate::common::{parse_and_compile, parse_and_run};

/// Reads the `max_length` header field (code units).
fn read_max_length(data_region: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data_region[offset], data_region[offset + 1]])
}

/// Reads the `cur_length` header field (code units).
fn read_cur_length(data_region: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data_region[offset + 2], data_region[offset + 3]])
}

/// Reads the `char_width` header field (1 = narrow, 2 = wide).
fn read_char_width(data_region: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data_region[offset + 4], data_region[offset + 5]])
}

/// Reads a WSTRING value as a `String`, decoding `cur_length` UTF-16LE code
/// units from the data region at `offset`.
fn read_wstring(data_region: &[u8], offset: usize) -> String {
    let cur_len = read_cur_length(data_region, offset) as usize;
    let data_start = offset + STRING_HEADER_BYTES;
    let units: Vec<u16> = (0..cur_len)
        .map(|i| {
            let b = data_start + i * 2;
            u16::from_le_bytes([data_region[b], data_region[b + 1]])
        })
        .collect();
    String::from_utf16(&units).unwrap()
}

/// Byte span of one `WSTRING[max_len]` element/variable in the data region.
fn wstring_region(max_len: usize) -> usize {
    STRING_HEADER_BYTES + max_len * 2
}

#[test]
fn wstring_when_literal_initializer_then_utf16le_bytes_and_wide_header() {
    let source = "
PROGRAM main
  VAR
    ws : WSTRING[10] := \"hi\";
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Header: max_len = 10 code units, cur_len = 2 code units, char_width = 2.
    assert_eq!(read_max_length(&bufs.data_region, 0), 10);
    assert_eq!(read_cur_length(&bufs.data_region, 0), 2);
    assert_eq!(read_char_width(&bufs.data_region, 0), 2);

    // Data: 'h' 'i' as UTF-16LE little-endian code units.
    assert_eq!(
        &bufs.data_region[STRING_HEADER_BYTES..STRING_HEADER_BYTES + 4],
        &[0x68, 0x00, 0x69, 0x00]
    );
    assert_eq!(read_wstring(&bufs.data_region, 0), "hi");
}

#[test]
fn wstring_when_non_ascii_bmp_literal_then_utf16le_code_units() {
    // U+00E9 (é) and U+20AC (€) are BMP code points needing the high byte.
    let source = "
PROGRAM main
  VAR
    ws : WSTRING[10] := \"é€\";
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_cur_length(&bufs.data_region, 0), 2);
    assert_eq!(read_char_width(&bufs.data_region, 0), 2);
    assert_eq!(
        &bufs.data_region[STRING_HEADER_BYTES..STRING_HEADER_BYTES + 4],
        &[0xE9, 0x00, 0xAC, 0x20]
    );
    assert_eq!(read_wstring(&bufs.data_region, 0), "é€");
}

#[test]
fn wstring_when_assigned_from_wstring_var_then_value_copied() {
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

    let dst_offset = wstring_region(10);
    assert_eq!(read_char_width(&bufs.data_region, dst_offset), 2);
    assert_eq!(read_cur_length(&bufs.data_region, dst_offset), 3);
    assert_eq!(read_wstring(&bufs.data_region, dst_offset), "abc");
}

#[test]
fn wstring_when_literal_assignment_statement_then_value_stored() {
    let source = "
PROGRAM main
  VAR
    ws : WSTRING[10];
  END_VAR
  ws := \"world\";
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_char_width(&bufs.data_region, 0), 2);
    assert_eq!(read_wstring(&bufs.data_region, 0), "world");
}

#[test]
fn wstring_when_compared_equal_then_eq_true_and_ne_false() {
    let source = "
PROGRAM main
  VAR
    a : WSTRING[10] := \"abc\";
    b : WSTRING[10] := \"abc\";
    eq : BOOL;
    ne : BOOL;
  END_VAR
  eq := a = b;
  ne := a <> b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // a = var0, b = var1, eq = var2, ne = var3. BOOL true = 1, false = 0.
    assert_eq!(bufs.vars[2].as_i32(), 1);
    assert_eq!(bufs.vars[3].as_i32(), 0);
}

#[test]
fn wstring_when_compared_different_then_eq_false_and_ne_true() {
    let source = "
PROGRAM main
  VAR
    a : WSTRING[10] := \"abc\";
    b : WSTRING[10] := \"abd\";
    eq : BOOL;
    ne : BOOL;
  END_VAR
  eq := a = b;
  ne := a <> b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[2].as_i32(), 0);
    assert_eq!(bufs.vars[3].as_i32(), 1);
}

#[test]
fn wstring_when_len_then_returns_code_unit_count() {
    let source = "
PROGRAM main
  VAR
    ws : WSTRING[10] := \"hello\";
    n : DINT;
  END_VAR
  n := LEN(ws);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // ws = var0, n = var1. LEN counts code units, not bytes.
    assert_eq!(bufs.vars[1].as_i32(), 5);
}

#[test]
fn wstring_when_concat_then_joins_code_units() {
    let source = "
PROGRAM main
  VAR
    a : WSTRING[10] := \"foo\";
    b : WSTRING[10] := \"bar\";
    out : WSTRING[20];
  END_VAR
  out := CONCAT(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // a, b are WSTRING[10] (26 bytes each); out is WSTRING[20] at offset 52.
    let out_offset = 2 * wstring_region(10);
    assert_eq!(read_char_width(&bufs.data_region, out_offset), 2);
    assert_eq!(read_cur_length(&bufs.data_region, out_offset), 6);
    assert_eq!(read_wstring(&bufs.data_region, out_offset), "foobar");
}

#[test]
fn wstring_when_left_right_mid_then_index_by_code_unit() {
    let source = "
PROGRAM main
  VAR
    s : WSTRING[20] := \"abcdef\";
    l : WSTRING[20];
    r : WSTRING[20];
    m : WSTRING[20];
  END_VAR
  l := LEFT(s, 2);
  r := RIGHT(s, 3);
  m := MID(s, 3, 2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let region = wstring_region(20);
    let l_offset = region;
    let r_offset = 2 * region;
    let m_offset = 3 * region;
    // LEFT(s,2)="ab"; RIGHT(s,3)="def"; MID(s,3,2)= 3 code units from pos 2 ="bcd".
    assert_eq!(read_wstring(&bufs.data_region, l_offset), "ab");
    assert_eq!(read_wstring(&bufs.data_region, r_offset), "def");
    assert_eq!(read_wstring(&bufs.data_region, m_offset), "bcd");
}

#[test]
fn wstring_when_find_substring_then_returns_code_unit_position() {
    let source = "
PROGRAM main
  VAR
    hay : WSTRING[20] := \"abcdef\";
    needle : WSTRING[20] := \"cd\";
    pos : DINT;
  END_VAR
  pos := FIND(hay, needle);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // hay = var0, needle = var1, pos = var2. FIND is 1-based by code unit.
    assert_eq!(bufs.vars[2].as_i32(), 3);
}

#[test]
fn wstring_array_when_assigned_and_read_back_then_values_match() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF WSTRING[8];
    result : WSTRING[8];
  END_VAR
  arr[1] := \"one\";
  arr[2] := \"two\";
  result := arr[2];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let arr_base = bufs.vars[0].as_i32() as usize;
    let stride = wstring_region(8);
    assert_eq!(read_char_width(&bufs.data_region, arr_base), 2);
    assert_eq!(read_wstring(&bufs.data_region, arr_base), "one");
    assert_eq!(read_wstring(&bufs.data_region, arr_base + stride), "two");

    // result is the scalar WSTRING after the 3-element array.
    let result_offset = arr_base + 3 * stride;
    assert_eq!(read_char_width(&bufs.data_region, result_offset), 2);
    assert_eq!(read_wstring(&bufs.data_region, result_offset), "two");
}

#[test]
fn wstring_array_when_initial_values_then_populated() {
    let source = "
PROGRAM main
  VAR
    days : ARRAY[1..3] OF WSTRING[8] := [\"Mon\", \"Tue\", \"Wed\"];
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let base = bufs.vars[0].as_i32() as usize;
    let stride = wstring_region(8);
    assert_eq!(read_char_width(&bufs.data_region, base), 2);
    assert_eq!(read_wstring(&bufs.data_region, base), "Mon");
    assert_eq!(read_wstring(&bufs.data_region, base + stride), "Tue");
    assert_eq!(read_wstring(&bufs.data_region, base + 2 * stride), "Wed");
}

#[test]
fn mixed_string_and_wstring_when_in_one_program_then_independent() {
    let source = "
PROGRAM main
  VAR
    narrow : STRING[10] := 'abc';
    wide : WSTRING[10] := \"abc\";
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // narrow at offset 0: char_width 1, one byte per char.
    assert_eq!(read_char_width(&bufs.data_region, 0), 1);
    assert_eq!(read_cur_length(&bufs.data_region, 0), 3);
    assert_eq!(
        &bufs.data_region[STRING_HEADER_BYTES..STRING_HEADER_BYTES + 3],
        &[b'a', b'b', b'c']
    );

    // wide follows narrow's region (6 + 10*1 = 16): char_width 2, UTF-16LE.
    let wide_offset = STRING_HEADER_BYTES + 10;
    assert_eq!(read_char_width(&bufs.data_region, wide_offset), 2);
    assert_eq!(read_cur_length(&bufs.data_region, wide_offset), 3);
    assert_eq!(
        &bufs.data_region[wide_offset + STRING_HEADER_BYTES..wide_offset + STRING_HEADER_BYTES + 6],
        &[0x61, 0x00, 0x62, 0x00, 0x63, 0x00]
    );
}

#[test]
fn wstring_when_declared_then_debug_tag_is_wstring() {
    let source = "
PROGRAM main
  VAR
    x : WSTRING[10] := \"hi\";
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();
    let var = debug.var_names.iter().find(|v| v.name == "x").unwrap();
    assert_eq!(var.type_name, "WSTRING");
    assert_eq!(var.iec_type_tag, iec_type_tag::WSTRING);
}

#[test]
fn string_assigned_wstring_when_analyzed_then_compile_error() {
    // STRING := WSTRING is rejected at compile time (P4034); the runtime
    // encoding-mismatch trap is only defense-in-depth.
    use ironplc_dsl::core::FileId;
    use ironplc_parser::parse_program;
    use ironplc_problems::Problem;

    let source = "
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  s := w;
END_PROGRAM
";
    let library =
        parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (_lib, context) =
        ironplc_analyzer::stages::analyze(&[&library], &CompilerOptions::default()).unwrap();
    assert!(
        context
            .diagnostics()
            .iter()
            .any(|d| d.code == Problem::StringEncodingMismatch.code()),
        "expected P4034 for STRING := WSTRING"
    );
}

