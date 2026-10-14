//! Tests of string assignments and of the standard string functions,
//! compiled from the lowered program, with every statement required to
//! lower.

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

/// Each string function that yields a string gives what the standard says.
/// A length is cut at the end of its string and a negative one counts as 0;
/// a position below 1 counts as 1, and one past the end as the end
/// (REQ-LOW-codegen-089). A result longer than its place keeps its first
/// code units (REQ-LOW-codegen-056).
#[test]
fn run_when_string_functions_yield_strings_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING := 'hello'; w : WSTRING := \"π ≈ 3.14\"; short : STRING[4];
               r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11 : STRING;
               w1, w2 : WSTRING; n : DINT; END_VAR
           r1 := CONCAT(s, ' world');
           r2 := LEFT(s, 2);
           r3 := RIGHT(s, 3);
           r4 := MID(s, 3, 2);
           r5 := INSERT(s, 'XY', 2);
           r6 := DELETE(s, 2, 3);
           r7 := REPLACE(s, 'J', 1, 1);
           r8 := LEFT(s, 9);
           r9 := LEFT(s, -1);
           r10 := MID(s, 2, 0);
           r11 := INSERT(s, '!', 9);
           short := CONCAT(s, s);
           n := LEN(CONCAT(LEFT(s, 2), RIGHT(s, 2)));
           w1 := CONCAT(w, \"!\");
           w2 := MID(w, 1, 3);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("r1"), "hello world");
    assert_eq!(snapshot.read("r2"), "he");
    assert_eq!(snapshot.read("r3"), "llo");
    assert_eq!(snapshot.read("r4"), "ell");
    assert_eq!(snapshot.read("r5"), "heXYllo");
    assert_eq!(snapshot.read("r6"), "heo");
    assert_eq!(snapshot.read("r7"), "Jello");
    assert_eq!(snapshot.read("r8"), "hello");
    assert_eq!(snapshot.read("r9"), "");
    assert_eq!(snapshot.read("r10"), "he");
    assert_eq!(snapshot.read("r11"), "hello!");
    assert_eq!(snapshot.read("short"), "hell");
    assert_eq!(snapshot.read_as::<i32>("n"), 4);
    assert_eq!(snapshot.read("w1"), "π ≈ 3.14!");
    assert_eq!(snapshot.read("w2"), "≈");
}

/// A call of a string function that yields a string compiles to the same
/// instructions and constants by both routes, its operands passed as `LEN`'s
/// are, and its result stored, or passed to another call in a slot of its
/// capacity.
#[rstest]
#[case::concat("s := CONCAT(s, t);")]
#[case::concat_literal("s := CONCAT(t, 'ab');")]
#[case::insert("s := INSERT(s, t, 2);")]
#[case::replace("s := REPLACE(s, t, 1, 2);")]
#[case::delete("s := DELETE(s, 1, 2);")]
#[case::left("s := LEFT(t, 2);")]
#[case::right("s := RIGHT(t, 2);")]
#[case::mid("s := MID(t, 2, 1);")]
#[case::nested("s := CONCAT(LEFT(s, 1), RIGHT(t, 1));")]
#[case::len_of_call("n := LEN(CONCAT(s, t));")]
#[case::wide_concat("w := CONCAT(w, \"x\");")]
#[case::wide_mid("w := MID(w, 1, 1);")]
fn compile_when_string_function_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR s : STRING; t : STRING[3]; w : WSTRING; n : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}
