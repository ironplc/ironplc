//! End-to-end integration tests for the MID standard function.

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
fn safe_string_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        (0x20u8..=0x7Eu8).prop_filter("exclude quote and dollar", |&b| b != b'\'' && b != b'$'),
        0..=254,
    )
    .prop_map(|bytes| bytes.into_iter().map(|b| b as char).collect())
}

// --- Deterministic anchors ---

#[test]
fn end_to_end_when_mid_beginning_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    result : STRING;
  END_VAR
  result := MID(s1, 5, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // MID 5 chars starting at position 1 -> 'Hello'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello");
}

#[test]
fn end_to_end_when_mid_position_beyond_end_then_empty() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABC';
    result : STRING;
  END_VAR
  result := MID(s1, 5, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Position 10 is beyond end of 3-char string -> empty
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "");
}

// --- Property test: MID(s, n, p) == n chars from 1-based position p ---
// Inputs are bounded to the unambiguous in-range domain (non-empty s, p a valid
// 1-based position, n within the remaining length). Oracle is pure Rust. The
// out-of-range/clamp branch is pinned by the deterministic anchor above.
proptest! {
    #[test]
    fn end_to_end_when_mid_of_arbitrary_string_then_takes_substring(
        (s, n, p) in safe_string_strategy()
            .prop_filter("non-empty", |s| !s.is_empty())
            .prop_flat_map(|s| {
                let len = s.chars().count();
                (Just(s), 1usize..=len)
            })
            .prop_flat_map(|(s, p)| {
                let len = s.chars().count();
                let max_n = len - p + 1;
                (Just(s), 0usize..=max_n, Just(p))
            }),
    ) {
        let expected: String = s.chars().skip(p - 1).take(n).collect();
        let source = format!(
            "
PROGRAM main
  VAR
    s1 : STRING := '{s}';
    result : STRING;
  END_VAR
  result := MID(s1, {n}, {p});
END_PROGRAM
"
        );
        let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
        let result_offset = string_offset(&[254]);
        prop_assert_eq!(read_string(&bufs.data_region, result_offset), expected);
    }
}
