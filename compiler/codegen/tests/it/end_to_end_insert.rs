//! End-to-end integration tests for the INSERT standard function.

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
/// Length is bounded to 0..=127 so a combined two-string result stays <= 254
/// and never triggers the STRING[254] truncation branch.
fn safe_string_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        (0x20u8..=0x7Eu8).prop_filter("exclude quote and dollar", |&b| b != b'\'' && b != b'$'),
        0..=127,
    )
    .prop_map(|bytes| bytes.into_iter().map(|b| b as char).collect())
}

// --- Deterministic anchors ---

#[test]
fn end_to_end_when_insert_in_middle_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'HelloWorld';
    s2 : STRING := ' ';
    result : STRING;
  END_VAR
  result := INSERT(s1, s2, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Insert ' ' after position 5: Hello + ' ' + World = 'Hello World'
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_insert_result_truncated_by_short_destination_then_truncates() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XXXXX';
    result : STRING[6];
  END_VAR
  result := INSERT(s1, s2, 2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Full result would be AB + XXXXX + CDE = ABXXXXXCDE (10 chars).
    // But result is STRING[6], so it truncates to 'ABXXXX' (6 chars).
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXXXX");
}

// --- Property test: INSERT(s1, s2, p) == insert s2 after 1-based position p ---
// Inputs are bounded so the combined result stays <= 254 (no truncation), and p
// is a valid 0..=len(s1) insertion point. Oracle is pure Rust. The truncation
// branch is pinned by the deterministic anchor above.
proptest! {
    #[test]
    fn end_to_end_when_insert_of_arbitrary_strings_then_splices(
        (s1, s2, p) in (safe_string_strategy(), safe_string_strategy())
            .prop_flat_map(|(s1, s2)| {
                let len = s1.chars().count();
                (Just(s1), Just(s2), 0usize..=len)
            }),
    ) {
        let expected: String = s1
            .chars()
            .take(p)
            .chain(s2.chars())
            .chain(s1.chars().skip(p))
            .collect();
        let source = format!(
            "
PROGRAM main
  VAR
    s1 : STRING := '{s1}';
    s2 : STRING := '{s2}';
    result : STRING;
  END_VAR
  result := INSERT(s1, s2, {p});
END_PROGRAM
"
        );
        let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
        let result_offset = string_offset(&[254, 254]);
        prop_assert_eq!(read_string(&bufs.data_region, result_offset), expected);
    }
}
