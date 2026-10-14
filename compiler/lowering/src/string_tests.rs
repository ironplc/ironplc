//! Tests of lowering strings.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower;

/// REQ-LOW-lowering-054: a string literal or the string at a place is
/// stored at a string place. A literal is in the encoding its delimiter
/// spells and as long as it is, and the string at a place has the place's
/// capacity, the default 254 where its declaration gives none.
#[test]
fn lower_statement_when_string_assigned_then_assign_str() {
    let outcome = lower(
        "PROGRAM main VAR s : STRING[5]; t : STRING; u : STRING; w : WSTRING; END_VAR
            s := 'abc';
            s := t;
            u := (t);
            w := \"xyz\";
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-str v0^.f0 (literal string[3] \"abc\")",
            "assign-str v0^.f0 (read string[254] v0^.f1)",
            "assign-str v0^.f2 (read string[254] v0^.f1)",
            "assign-str v0^.f3 (literal wstring[3] \"xyz\")",
        ]
    );
}

/// REQ-LOW-analyzer-053: the string at a place has the capacity its
/// declaration gives, as a variable or a structure's field.
#[test]
fn lower_statement_when_sized_string_read_then_capacity_of_its_declaration() {
    let outcome = lower(
        "TYPE Named : STRUCT n : WSTRING[4]; END_STRUCT; END_TYPE
         PROGRAM main VAR s : STRING[5]; t : STRING; w : WSTRING; r : Named; END_VAR
            t := s;
            w := r.n;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-str v0^.f1 (read string[5] v0^.f0)",
            "assign-str v0^.f2 (read wstring[4] v0^.f3.f0)",
        ]
    );
}

/// REQ-LOW-lowering-147: `LEN` and `FIND` are calls of the intrinsic for
/// the encoding of their strings, and yield an `INT`. A literal argument has
/// its own length as its capacity.
#[test]
fn lower_statement_when_len_or_find_then_intrinsic_of_encoding() {
    let outcome = lower(
        "PROGRAM main VAR s : STRING[5]; w : WSTRING; n : DINT; END_VAR
            n := LEN(s);
            n := FIND(s, 'ab');
            n := LEN(\"xyz\");
            n := FIND(w, w);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (call len_string i32s (read string[5] v0^.f0))",
            "assign v0^.f2 (call find_string i32s (read string[5] v0^.f0) (literal string[2] \"ab\"))",
            "assign v0^.f2 (call len_wstring i32s (literal wstring[3] \"xyz\"))",
            "assign v0^.f2 (call find_wstring i32s (read wstring[254] v0^.f1) (read wstring[254] v0^.f1))",
        ]
    );
}

/// REQ-LOW-lowering-167: a string function that yields a string is a call of
/// the intrinsic for its encoding, with room for every code unit of its
/// strings, and is itself a string argument of another call.
#[test]
fn lower_statement_when_string_function_yields_string_then_call_of_capacity() {
    let outcome = lower(
        "PROGRAM main VAR s : STRING[5]; t : STRING; w : WSTRING[4]; n : DINT; END_VAR
            t := CONCAT(s, 'ab');
            t := MID(s, 2, 1);
            n := LEN(LEFT(s, 2));
            w := INSERT(w, \"xy\", 1);
            t := CONCAT(CONCAT(s, s), 'c');
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-str v0^.f1 (call concat_string string[7] (read string[5] v0^.f0) (literal string[2] \"ab\"))",
            "assign-str v0^.f1 (call mid_string string[5] (read string[5] v0^.f0) (const i32s 2) (const i32s 1))",
            "assign v0^.f3 (call len_string i32s (call left_string string[5] (read string[5] v0^.f0) (const i32s 2)))",
            "assign-str v0^.f2 (call insert_wstring wstring[6] (read wstring[4] v0^.f2) (literal wstring[2] \"xy\") (const i32s 1))",
            "assign-str v0^.f1 (call concat_string string[11] (call concat_string string[10] (read string[5] v0^.f0) (read string[5] v0^.f0)) (literal string[1] \"c\"))",
        ]
    );
}

/// A string conversion and a comparison of strings are not lowered yet.
#[test]
fn lower_statement_when_string_conversion_or_comparison_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR s : STRING; x : BOOL; n : DINT; END_VAR
            s := DINT_TO_STRING(n);
            x := s = 'abc';
            n := STRING_TO_DINT(s);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["unsupported", "unsupported", "unsupported"]
    );
}
