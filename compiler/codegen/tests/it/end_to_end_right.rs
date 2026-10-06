//! End-to-end integration tests for the RIGHT standard function.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;
use proptest::prelude::*;

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
fn end_to_end_when_right_partial_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    result : STRING;
  END_VAR
  result := RIGHT(s1, 5);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    // RIGHT 5 chars of 'Hello World' -> 'World'
    assert_eq!(snapshot.read("result"), "World");
}

#[test]
fn end_to_end_when_right_exceeds_length_then_entire_string() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hi';
    result : STRING;
  END_VAR
  result := RIGHT(s1, 100);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    // RIGHT 100 chars of 'Hi' -> 'Hi' (clamped to string length)
    assert_eq!(snapshot.read("result"), "Hi");
}

// --- Property test: RIGHT(s, n) == last min(n, len(s)) characters ---
// Oracle is pure Rust, independent of the VM implementation. The two anchors
// above pin the nominal and clamp branches deterministically.
proptest! {
    #[test]
    fn end_to_end_when_right_of_arbitrary_string_then_takes_suffix(
        s in safe_string_strategy(),
        n in 0usize..=260,
    ) {
        let l = s.chars().count();
        let expected: String = s.chars().skip(l.saturating_sub(n)).collect();
        let source = format!(
            "
PROGRAM main
  VAR
    s1 : STRING := '{s}';
    result : STRING;
  END_VAR
  result := RIGHT(s1, {n});
END_PROGRAM
"
        );
        let snapshot = Snapshot::run(&source, &CompilerOptions::default());
        prop_assert_eq!(snapshot.read("result"), expected);
    }
}
