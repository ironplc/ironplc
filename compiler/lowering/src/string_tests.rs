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

/// A string function and a comparison of strings are not lowered yet.
#[test]
fn lower_statement_when_string_function_or_comparison_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR s : STRING; x : BOOL; n : DINT; END_VAR
            s := CONCAT(s, 'x');
            x := s = 'abc';
            n := LEN(s);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["unsupported", "unsupported", "unsupported"]
    );
}
