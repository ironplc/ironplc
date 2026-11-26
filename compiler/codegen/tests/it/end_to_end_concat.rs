//! End-to-end integration tests for the CONCAT standard function.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;
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
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("result"), "Hello World");
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
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("result"), "Hello World");
}

#[test]
fn end_to_end_when_concat_three_or_four_strings_then_joins_in_order() {
    // CONCAT is extensible: further inputs append in order, as the nested
    // two-input form does.
    let source = "
PROGRAM main
  VAR
    a : STRING := 'ab';
    b : STRING := 'CD';
    c : STRING := 'ef';
    d : STRING := 'GH';
    joined3 : STRING;
    joined4 : STRING;
    nested : STRING;
  END_VAR
  joined3 := CONCAT(a, b, c);
  joined4 := CONCAT(a, b, c, d);
  nested := CONCAT(CONCAT(a, b), c);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("joined3"), "abCDef");
    assert_eq!(snapshot.read("joined4"), "abCDefGH");
    assert_eq!(snapshot.read("nested"), "abCDef");
}

#[test]
fn end_to_end_when_concat_named_extended_inputs_then_joins_in_number_order() {
    let source = "
PROGRAM main
  VAR
    result : STRING;
  END_VAR
  result := CONCAT(IN3 := 'c', IN1 := 'a', IN2 := 'b');
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("result"), "abc");
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
        let snapshot = Snapshot::run(&source, &CompilerOptions::default());
        prop_assert_eq!(snapshot.read("result"), expected);
    }
}
