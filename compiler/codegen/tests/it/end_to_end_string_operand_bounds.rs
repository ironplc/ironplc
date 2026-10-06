//! End-to-end tests for string operands that are wider than the default
//! capacity and are not plain named variables: a literal and a nested call.
//!
//! Such an operand is copied into a temporary data-region slot before the
//! operation reads it. A slot sized at the 254-code-unit default cut a longer
//! operand on the way in, so `LEN` measured the copy, a comparison judged a
//! value unequal to its own source, and `FIND` missed a character past the
//! cut. The slot is now sized at the operand's own bound. Declared operands
//! -- an array element or a structure field -- are covered by
//! `end_to_end_len.rs`.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

/// Compiles and runs `source`, returning the value of `name` as an i32.
fn run_i32(source: &str, name: &str) -> i32 {
    Snapshot::run(source, &CompilerOptions::default()).read_as::<i32>(name)
}

/// A value of `len` code units, all the same character.
fn repeated(len: usize) -> String {
    "a".repeat(len)
}

#[test]
fn end_to_end_when_len_of_nested_concat_then_returns_full_result_length() {
    let source = format!(
        "
PROGRAM main
  VAR
    c128 : STRING[128] := '{half}';
    n : INT;
  END_VAR
  n := LEN(CONCAT(c128, c128));
END_PROGRAM
",
        half = repeated(128),
    );

    assert_eq!(run_i32(&source, "n"), 256);
}

#[test]
fn end_to_end_when_len_of_nested_wide_concat_then_returns_full_result_length() {
    let source = format!(
        "
PROGRAM main
  VAR
    w128 : WSTRING[128] := \"{half}\";
    n : INT;
  END_VAR
  n := LEN(CONCAT(w128, w128));
END_PROGRAM
",
        half = repeated(128),
    );

    assert_eq!(run_i32(&source, "n"), 256);
}

#[test]
fn end_to_end_when_comparing_variable_with_nested_concat_then_equal() {
    let source = format!(
        "
PROGRAM main
  VAR
    c128 : STRING[128] := '{half}';
    c256 : STRING[300];
    eq : BOOL;
    ne : BOOL;
  END_VAR
  c256 := CONCAT(c128, c128);
  eq := c256 = CONCAT(c128, c128);
  ne := c256 <> CONCAT(c128, c128);
END_PROGRAM
",
        half = repeated(128),
    );

    let snapshot = Snapshot::run(&source, &CompilerOptions::default());

    assert_eq!(snapshot.read_as::<i32>("eq"), 1, "eq");
    assert_eq!(snapshot.read_as::<i32>("ne"), 0, "ne");
}

// The only declared string is STRING[10], so nothing but the literal itself
// can make room for its 300 code units.
#[test]
fn end_to_end_when_len_of_literal_longer_than_every_declaration_then_returns_its_length() {
    let source = format!(
        "
PROGRAM main
  VAR
    s : STRING[10];
    n : INT;
  END_VAR
  s := 'x';
  n := LEN('{value}');
END_PROGRAM
",
        value = repeated(300),
    );

    assert_eq!(run_i32(&source, "n"), 300);
}

#[test]
fn end_to_end_when_comparing_variable_with_long_literal_then_equal() {
    let source = format!(
        "
PROGRAM main
  VAR
    s : STRING[300] := '{value}';
    same : BOOL;
  END_VAR
  same := s = '{value}';
END_PROGRAM
",
        value = repeated(300),
    );

    assert_eq!(run_i32(&source, "same"), 1);
}

// The 'z' sits at position 257, past the default capacity.
#[test]
fn end_to_end_when_find_in_long_array_element_then_returns_position_past_default() {
    let source = format!(
        "
PROGRAM main
  VAR
    lines : ARRAY[0..1] OF STRING[300];
    n : INT;
  END_VAR
  lines[0] := '{prefix}z';
  n := FIND(lines[0], 'z');
END_PROGRAM
",
        prefix = repeated(256),
    );

    assert_eq!(run_i32(&source, "n"), 257);
}

// A wider temporary changes nothing about the destination: a literal
// assigned into STRING[10] is still cut to the destination's capacity.
#[test]
fn end_to_end_when_long_literal_assigned_to_short_string_then_destination_still_truncates() {
    let source = format!(
        "
PROGRAM main
  VAR
    s : STRING[10];
  END_VAR
  s := '{value}';
END_PROGRAM
",
        value = repeated(300),
    );

    let snapshot = Snapshot::run(&source, &CompilerOptions::default());

    assert_eq!(snapshot.read("s"), repeated(10));
}

// The call's temporary takes the function's declared return capacity, so the
// 256-unit result is not cut to the default.
#[test]
fn end_to_end_when_len_of_user_function_result_then_returns_declared_result_length() {
    let source = format!(
        "
FUNCTION doubled : STRING[300]
  VAR_INPUT
    half : STRING[128];
  END_VAR
  doubled := CONCAT(half, half);
END_FUNCTION

PROGRAM main
  VAR
    c128 : STRING[128] := '{half}';
    n : INT;
  END_VAR
  n := LEN(doubled(c128));
END_PROGRAM
",
        half = repeated(128),
    );

    assert_eq!(run_i32(&source, "n"), 256);
}
