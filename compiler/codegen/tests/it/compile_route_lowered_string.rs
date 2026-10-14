//! Tests of string assignments and of the string functions that yield a
//! number, compiled from the lowered program, with every statement required
//! to lower.

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

/// `LEN` counts the code units of its string, and `FIND` gives the 1-based
/// position of the first occurrence of its second string in its first, or 0
/// when there is none or the string it looks for is empty
/// (REQ-LOW-codegen-089). A `WSTRING` counts 16-bit code units.
#[test]
fn run_when_len_or_find_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING[8] := 'hello'; w : WSTRING := \"π ≈ 3.14\";
               r1, r2, r3, r4, r5, r6, r7 : DINT; END_VAR
           r1 := LEN(s);
           r2 := LEN('');
           r3 := FIND(s, 'll');
           r4 := FIND(s, 'xyz');
           r5 := FIND(s, '');
           r6 := LEN(w);
           r7 := FIND(w, \"3.14\");
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("r1"), 5);
    assert_eq!(snapshot.read_as::<i32>("r2"), 0);
    assert_eq!(snapshot.read_as::<i32>("r3"), 3);
    assert_eq!(snapshot.read_as::<i32>("r4"), 0);
    assert_eq!(snapshot.read_as::<i32>("r5"), 0);
    assert_eq!(snapshot.read_as::<i32>("r6"), 8);
    assert_eq!(snapshot.read_as::<i32>("r7"), 5);
}

/// A call of `LEN` or `FIND` compiles to the same instructions and constants
/// by both routes: the string at a variable is passed as the variable's own
/// slot, and any other string is first stored in a slot of its capacity.
#[rstest]
#[case::len_of_variable("n := LEN(s);")]
#[case::len_of_literal("n := LEN('abc');")]
#[case::find_of_variables("n := FIND(s, t);")]
#[case::find_of_literal("n := FIND(s, 'b');")]
#[case::wide_len("n := LEN(w);")]
#[case::wide_find("n := FIND(w, \"y\");")]
fn compile_when_len_or_find_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR s : STRING; t : STRING[3]; w : WSTRING; n : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}
