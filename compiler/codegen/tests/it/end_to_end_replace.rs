//! End-to-end integration tests for the REPLACE standard function.

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
fn end_to_end_when_replace_middle_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'Earth';
    result : STRING;
  END_VAR
  result := REPLACE(s1, s2, 5, 7);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Offsets are STRING_HEADER_BYTES-relative (see string_offset): s1 at 0,
    // s2 after s1, result after s2.
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello Earth");
}

#[test]
fn end_to_end_when_replace_result_truncated_by_short_destination_then_truncates() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XXXXX';
    result : STRING[6];
  END_VAR
  result := REPLACE(s1, s2, 1, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Full result would be AB + XXXXX + DE = ABXXXXXDE (9 chars).
    // But result is STRING[6], so it truncates to 'ABXXXX' (6 chars).
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXXXX");
}

// --- Property test: REPLACE(s1, s2, n, p) == replace n chars of s1 from
// 1-based position p with s2. Inputs are bounded so the replaced range lies
// fully inside s1 and the combined result stays <= 254 (no truncation). Oracle
// is pure Rust. The truncation branch is pinned by the anchor above.
proptest! {
    #[test]
    fn end_to_end_when_replace_of_arbitrary_strings_then_substitutes(
        (s1, s2, n, p) in (safe_string_strategy(), safe_string_strategy())
            .prop_filter("non-empty s1", |(s1, _)| !s1.is_empty())
            .prop_flat_map(|(s1, s2)| {
                let len = s1.chars().count();
                (Just(s1), Just(s2), Just(len), 1usize..=len)
            })
            .prop_flat_map(|(s1, s2, len, p)| {
                let max_n = len - p + 1;
                (Just(s1), Just(s2), 0usize..=max_n, Just(p))
            }),
    ) {
        let c: Vec<char> = s1.chars().collect();
        let a = p - 1;
        let b = a + n;
        let expected: String = c[..a]
            .iter()
            .chain(s2.chars().collect::<Vec<char>>().iter())
            .chain(c[b..].iter())
            .collect();
        let source = format!(
            "
PROGRAM main
  VAR
    s1 : STRING := '{s1}';
    s2 : STRING := '{s2}';
    result : STRING;
  END_VAR
  result := REPLACE(s1, s2, {n}, {p});
END_PROGRAM
"
        );
        let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
        let result_offset = string_offset(&[254, 254]);
        prop_assert_eq!(read_string(&bufs.data_region, result_offset), expected);
    }
}
