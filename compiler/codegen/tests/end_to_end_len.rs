//! End-to-end integration tests for the LEN standard function.

mod common;

use common::parse_and_run;
use proptest::prelude::*;

#[test]
fn end_to_end_when_len_of_string_with_value_then_returns_length() {
    let source = "
PROGRAM main
  VAR
    s : STRING := 'hello';
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // s is at variable slot 0, n is at variable slot 1.
    assert_eq!(bufs.vars[1].as_i32(), 5);
}

#[test]
fn end_to_end_when_len_of_empty_string_then_returns_zero() {
    let source = "
PROGRAM main
  VAR
    s : STRING;
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_len_of_string_with_max_length_then_returns_current_length() {
    let source = "
PROGRAM main
  VAR
    s : STRING[10] := 'hi';
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // Current length is 2 ('hi'), not the max length of 10.
    assert_eq!(bufs.vars[1].as_i32(), 2);
}

#[test]
fn end_to_end_when_len_of_single_char_string_then_returns_one() {
    let source = "
PROGRAM main
  VAR
    s : STRING := 'x';
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[1].as_i32(), 1);
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

proptest! {
    #[test]
    fn end_to_end_when_len_of_arbitrary_string_then_returns_correct_length(
        s in safe_string_strategy()
    ) {
        let expected_len = s.len() as i32;
        let source = format!(
            "
PROGRAM main
  VAR
    s : STRING := '{}';
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
",
            s
        );
        let (_c, bufs) = parse_and_run(&source);

        prop_assert_eq!(bufs.vars[1].as_i32(), expected_len);
    }
}
