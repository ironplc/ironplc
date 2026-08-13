//! End-to-end integration tests for the DELETE standard function.

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
fn end_to_end_when_delete_middle_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    result : STRING;
  END_VAR
  result := DELETE(s1, 6, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 6 chars starting at position 1: remove 'Hello ' -> 'World'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "World");
}

#[test]
fn end_to_end_when_delete_exceeds_length_then_deletes_to_end() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := DELETE(s1, 100, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 100 chars starting at position 3, but only 3 chars remain: remove 'CDE' -> 'AB'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "AB");
}

// --- Property test: DELETE(s, n, p) == remove n chars from 1-based position p ---
// Inputs are bounded to a non-empty s with a valid 1-based position p; n may
// exceed the remaining length (the oracle clamps like the VM). Oracle is pure
// Rust. The clamp branch is also pinned by the deterministic anchor above.
proptest! {
    #[test]
    fn end_to_end_when_delete_of_arbitrary_string_then_removes_range(
        (s, n, p) in safe_string_strategy()
            .prop_filter("non-empty", |s| !s.is_empty())
            .prop_flat_map(|s| {
                let len = s.chars().count();
                (Just(s), 0usize..=260, 1usize..=len)
            }),
    ) {
        let c: Vec<char> = s.chars().collect();
        let a = p - 1;
        let b = (a + n).min(c.len());
        let expected: String = c[..a].iter().chain(c[b..].iter()).collect();
        let source = format!(
            "
PROGRAM main
  VAR
    s1 : STRING := '{s}';
    result : STRING;
  END_VAR
  result := DELETE(s1, {n}, {p});
END_PROGRAM
"
        );
        let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
        let result_offset = string_offset(&[254]);
        prop_assert_eq!(read_string(&bufs.data_region, result_offset), expected);
    }
}
