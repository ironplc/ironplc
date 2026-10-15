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
/// declaration gives, as a variable, a structure's field or an array's
/// element.
#[test]
fn lower_statement_when_sized_string_read_then_capacity_of_its_declaration() {
    let outcome = lower(
        "TYPE Named : STRUCT n : WSTRING[4]; END_STRUCT; END_TYPE
         PROGRAM main VAR s : STRING[5]; t : STRING; w : WSTRING; r : Named;
             a : ARRAY[1..3] OF STRING[8]; END_VAR
            t := s;
            w := r.n;
            t := a[2];
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-str v0^.f1 (read string[5] v0^.f0)",
            "assign-str v0^.f2 (read wstring[4] v0^.f3.f0)",
            "assign-str v0^.f1 (read string[8] v0^.f4[(const i32s 2)])",
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

/// REQ-LOW-lowering-073, REQ-LOW-lowering-168: a conversion from a `STRING`
/// reads a number of the call's type, a bit string as the unsigned integer of
/// its width, as the program's parse policy says. A conversion to a
/// `STRING` writes a number of its operation type into a string of the
/// default capacity.
#[test]
fn lower_statement_when_string_conversion_then_intrinsic_of_number_type() {
    let outcome = lower(
        "PROGRAM main VAR s : STRING; n : DINT; u : UDINT; w : WORD; r : LREAL; i : INT; END_VAR
            n := STRING_TO_DINT(s);
            w := STRING_TO_WORD('7');
            r := STRING_TO_LREAL(s);
            s := DINT_TO_STRING(n);
            s := UDINT_TO_STRING(u);
            s := CONCAT(INT_TO_STRING(i), 'x');
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f1 (call string_to_i32[reject,trap] i32s (read string[254] v0^.f0))",
            "assign v0^.f3 (truncate 16 (call string_to_u16[reject,trap] i32u (literal string[1] \"7\")))",
            "assign v0^.f4 (call string_to_f64[reject,trap] f64 (read string[254] v0^.f0))",
            "assign-str v0^.f0 (call i32_to_string string[254] (read i32s v0^.f1))",
            "assign-str v0^.f0 (call u32_to_string string[254] (read i32u v0^.f2))",
            "assign-str v0^.f0 (call concat_string string[255] (call i32_to_string string[254] (read i32s v0^.f5)) (literal string[1] \"x\"))",
        ]
    );
}

/// REQ-LOW-lowering-169: a comparison of two strings compares with zero the
/// order the intrinsic of their encoding gives them, in the operator and the
/// function form (REQ-LOW-lowering-101), whichever string is a literal.
#[test]
fn lower_statement_when_comparison_of_strings_then_order_compared_with_zero() {
    let outcome = lower(
        "PROGRAM main VAR s : STRING[5]; w : WSTRING; x : BOOL; END_VAR
            x := s = 'abc';
            x := LT(s, CONCAT(s, s));
            x := 'abc' <> s;
            x := w >= \"xy\";
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (eq (call compare_string i32s (read string[5] v0^.f0) (literal string[3] \"abc\")) (const i32s 0))",
            "assign v0^.f2 (lt (call compare_string i32s (read string[5] v0^.f0) (call concat_string string[10] (read string[5] v0^.f0) (read string[5] v0^.f0))) (const i32s 0))",
            "assign v0^.f2 (ne (call compare_string i32s (literal string[3] \"abc\") (read string[5] v0^.f0)) (const i32s 0))",
            "assign v0^.f2 (ge (call compare_wstring i32s (read wstring[254] v0^.f1) (literal wstring[2] \"xy\")) (const i32s 0))",
        ]
    );
}
