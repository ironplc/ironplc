//! End-to-end integration tests for the CONCAT standard function.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;
use ironplc_container::STRING_HEADER_BYTES;
use proptest::prelude::*;

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

/// Computes the data_offset of a STRING variable given its position
/// in the declaration order and preceding string max lengths.
/// Each STRING variable occupies STRING_HEADER_BYTES + max_length bytes.
fn string_offset(preceding_max_lengths: &[u16]) -> usize {
    preceding_max_lengths
        .iter()
        .map(|&ml| STRING_HEADER_BYTES + ml as usize)
        .sum()
}

/// Generates printable ASCII strings safe for IEC 61131-3 string literals.
/// Excludes single quote (0x27) and dollar sign (0x24, the escape character).
/// Length is bounded to 0..=127 so the concatenated result stays <= 254 and
/// never triggers the STRING[254] truncation branch.
fn safe_string_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        (0x20u8..=0x7Eu8).prop_filter("exclude quote and dollar", |&b| b != b'\'' && b != b'$'),
        0..=127,
    )
    .prop_map(|bytes| bytes.into_iter().map(|b| b as char).collect())
}

// --- Deterministic anchors ---

#[test]
fn end_to_end_when_concat_two_strings_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    s2 : STRING := ' World';
    result : STRING;
  END_VAR
  result := CONCAT(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_concat_two_literals_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    result : STRING;
  END_VAR
  result := CONCAT('Hello', ' World');
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // result is the first (and only) declared string variable at offset 0.
    let result_offset = string_offset(&[]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

// --- Property test: CONCAT(s1, s2) == s1 followed by s2 ---
// Inputs are bounded so the concatenation stays <= 254 (no truncation). Oracle
// is pure Rust. The literal-argument lowering path is pinned by the anchor above.
proptest! {
    #[test]
    fn end_to_end_when_concat_of_arbitrary_strings_then_appends(
        s1 in safe_string_strategy(),
        s2 in safe_string_strategy(),
    ) {
        let expected = format!("{s1}{s2}");
        let source = format!(
            "
PROGRAM main
  VAR
    s1 : STRING := '{s1}';
    s2 : STRING := '{s2}';
    result : STRING;
  END_VAR
  result := CONCAT(s1, s2);
END_PROGRAM
"
        );
        let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
        let result_offset = string_offset(&[254, 254]);
        prop_assert_eq!(read_string(&bufs.data_region, result_offset), expected);
    }
}
