//! End-to-end integration tests for ARRAY OF STRING[N] support.

mod common;
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

#[test]
fn array_of_string_when_assign_then_stores_value() {
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..3] OF STRING[10];
  END_VAR
  names[1] := 'hello';
  names[2] := 'world';
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // names is var 0; its slot holds the base data_offset.
    let base_offset = bufs.vars[0].as_i32() as usize;
    let stride = STRING_HEADER_BYTES + 10; // 4 + 10 = 14 bytes per element

    assert_eq!(read_string(&bufs.data_region, base_offset), "hello");
    assert_eq!(
        read_string(&bufs.data_region, base_offset + stride),
        "world"
    );
    // Element 3 was not assigned — should be empty (cur_len = 0).
    assert_eq!(read_string(&bufs.data_region, base_offset + 2 * stride), "");
}

#[test]
fn array_of_string_when_read_back_then_value_matches() {
    // Assign a string, then copy it to a scalar STRING variable.
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF STRING[10];
    result : STRING[10];
  END_VAR
  arr[2] := 'test';
  result := arr[2];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // arr is var 0, result is var 1.
    // arr occupies 3 * (4 + 10) = 42 bytes in data region.
    // result starts at offset 42.
    let arr_base = bufs.vars[0].as_i32() as usize;
    let stride = STRING_HEADER_BYTES + 10;
    let result_offset = arr_base + 3 * stride;

    assert_eq!(read_string(&bufs.data_region, result_offset), "test");
}

#[test]
fn array_of_string_when_initial_values_then_populated() {
    let source = "
PROGRAM main
  VAR
    days : ARRAY[1..3] OF STRING[10] := ['Mon', 'Tue', 'Wed'];
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let base_offset = bufs.vars[0].as_i32() as usize;
    let stride = STRING_HEADER_BYTES + 10;

    assert_eq!(read_string(&bufs.data_region, base_offset), "Mon");
    assert_eq!(read_string(&bufs.data_region, base_offset + stride), "Tue");
    assert_eq!(
        read_string(&bufs.data_region, base_offset + 2 * stride),
        "Wed"
    );
}

#[test]
fn array_of_string_when_multidim_then_correct_indexing() {
    let source = "
PROGRAM main
  VAR
    grid : ARRAY[1..2, 1..2] OF STRING[5];
  END_VAR
  grid[1, 1] := 'a';
  grid[1, 2] := 'bb';
  grid[2, 1] := 'ccc';
  grid[2, 2] := 'dddd';
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let base_offset = bufs.vars[0].as_i32() as usize;
    let stride = STRING_HEADER_BYTES + 5; // 9 bytes per element

    // Flat layout: [1,1]=0, [1,2]=1, [2,1]=2, [2,2]=3
    assert_eq!(read_string(&bufs.data_region, base_offset), "a");
    assert_eq!(read_string(&bufs.data_region, base_offset + stride), "bb");
    assert_eq!(
        read_string(&bufs.data_region, base_offset + 2 * stride),
        "ccc"
    );
    assert_eq!(
        read_string(&bufs.data_region, base_offset + 3 * stride),
        "dddd"
    );
}

#[test]
fn array_of_string_when_truncated_then_respects_max_length() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..2] OF STRING[3];
  END_VAR
  arr[1] := 'abcdefgh';
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let base_offset = bufs.vars[0].as_i32() as usize;
    // Max length is 3, so 'abcdefgh' should be truncated to 'abc'.
    assert_eq!(read_string(&bufs.data_region, base_offset), "abc");
}

#[test]
fn array_of_string_when_default_length_then_uses_254() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..2] OF STRING;
  END_VAR
  arr[1] := 'hello';
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let base_offset = bufs.vars[0].as_i32() as usize;
    let stride = STRING_HEADER_BYTES + 254; // default max length

    assert_eq!(read_string(&bufs.data_region, base_offset), "hello");
    assert_eq!(read_string(&bufs.data_region, base_offset + stride), "");
}
