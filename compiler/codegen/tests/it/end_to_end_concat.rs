//! End-to-end integration tests for the CONCAT standard function.

use ironplc_parser::options::CompilerOptions;

use crate::common::{parse_and_run, read_string, string_offset};
use proptest::prelude::*;

/// Generates printable ASCII strings safe for IEC 61131-3 string literals.
/// Excludes single quote (0x27) and dollar sign (0x24, the escape character).
/// Length runs to the full 254 a bare STRING holds, so the concatenation
/// reaches 508: wider than any operand and than the default capacity, which
/// is what pins the result to a temporary sized from the operands.
fn safe_string_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        (0x20u8..=0x7Eu8).prop_filter("exclude quote and dollar", |&b| b != b'\'' && b != b'$'),
        0..=254,
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
// The result is declared STRING[508] so no destination truncates it. Oracle
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
    result : STRING[508];
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
