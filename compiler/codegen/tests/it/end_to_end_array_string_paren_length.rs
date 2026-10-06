//! End-to-end integration tests for `ARRAY[..] OF STRING(n)` — the
//! parenthesis length delimiter used as an array element type.
//!
//! The parenthesis form is a vendor extension gated behind
//! `allow_paren_string_length` (see
//! `parser/src/rule_token_no_paren_string_length.rs`). These tests pin that
//! the element-type position declares the length the bracket form does: a
//! length that was parsed but dropped would silently fall back to the default
//! 254, so the cases assign values longer than the declared length and check
//! that they are cut to it.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

fn paren_string_length_options() -> CompilerOptions {
    CompilerOptions {
        allow_paren_string_length: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn array_of_string_paren_length_when_assign_then_stores_value() {
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..3] OF STRING(10);
    r1 : STRING[20];
    r2 : STRING[20];
    r3 : STRING[20];
  END_VAR
  names[1] := 'hello';
  names[2] := 'world';
  names[3] := 'abcdefghijklmnop';
  r1 := names[1];
  r2 := names[2];
  r3 := names[3];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &paren_string_length_options());

    assert_eq!(snapshot.read("r1"), "hello");
    assert_eq!(snapshot.read("r2"), "world");
    assert_eq!(snapshot.read("r3"), "abcdefghij");
}

#[test]
fn array_of_wstring_paren_length_when_assign_then_stores_value() {
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..2] OF WSTRING(8);
    r1 : WSTRING[20];
    r2 : WSTRING[20];
  END_VAR
  names[1] := \"hi\";
  names[2] := \"there and more\";
  r1 := names[1];
  r2 := names[2];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &paren_string_length_options());

    assert_eq!(snapshot.read("r1"), "hi");
    assert_eq!(snapshot.read("r2"), "there an");
}

#[test]
fn array_of_string_paren_length_when_truncated_then_respects_max_length() {
    // The copy target is longer than the element, so only the parenthesised
    // length can cut the value.
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..2] OF STRING(3);
    r : STRING[10];
  END_VAR
  arr[1] := 'abcdefgh';
  r := arr[1];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &paren_string_length_options());

    assert_eq!(snapshot.read("r"), "abc");
}

#[test]
fn array_of_string_paren_length_when_initial_values_then_populated() {
    let source = "
PROGRAM main
  VAR
    days : ARRAY[1..3] OF STRING(10) := ['Mon', 'Tue', 'Wed'];
    r1 : STRING[10];
    r2 : STRING[10];
    r3 : STRING[10];
  END_VAR
  r1 := days[1];
  r2 := days[2];
  r3 := days[3];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &paren_string_length_options());

    assert_eq!(snapshot.read("r1"), "Mon");
    assert_eq!(snapshot.read("r2"), "Tue");
    assert_eq!(snapshot.read("r3"), "Wed");
}
