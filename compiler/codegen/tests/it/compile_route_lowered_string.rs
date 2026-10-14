//! Tests of string assignments compiled from the lowered program, with every
//! statement required to lower.

use rstest::rstest;

use crate::compile_route_lowered::{assert_same_code, run_lowered};

/// A literal and the string at a place are stored at a string place. A
/// value longer than the place keeps its first code units
/// (REQ-LOW-codegen-056), and a `WSTRING` keeps a character a `STRING`
/// cannot hold.
#[test]
fn run_when_strings_assigned_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING[5]; t : STRING := 'hello world'; u : STRING; v : STRING;
               w : WSTRING; END_VAR
           s := t;
           u := 'abc';
           v := s;
           w := \"π ≈ 3.14\";
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("s"), "hello");
    assert_eq!(snapshot.read("u"), "abc");
    assert_eq!(snapshot.read("v"), "hello");
    assert_eq!(snapshot.read("w"), "π ≈ 3.14");
}

/// A string assignment compiles to the same instructions and constants by
/// both routes.
#[rstest]
#[case::literal("s := 'abc';")]
#[case::empty_literal("s := '';")]
#[case::variable("s := t;")]
#[case::to_shorter("n := t;")]
#[case::wide_literal("w := \"xyz\";")]
#[case::wide_variable("w := x;")]
fn compile_when_string_assignment_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR s : STRING; t : STRING; n : STRING[3]; w : WSTRING; x : WSTRING; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}
