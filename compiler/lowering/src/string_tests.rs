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
            n := LEN(w);
            n := FIND(w, \"xyz\");
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (call len_string i32s (read string[5] v0^.f0))",
            "assign v0^.f2 (call find_string i32s (read string[5] v0^.f0) (literal string[2] \"ab\"))",
            "assign v0^.f2 (call len_wstring i32s (read wstring[254] v0^.f1))",
            "assign v0^.f2 (call find_wstring i32s (read wstring[254] v0^.f1) (literal wstring[3] \"xyz\"))",
        ]
    );
}

/// REQ-LOW-lowering-205: `LEN` of a literal, in either encoding, is a
/// constant of its length in code units, the type `LEN` yields; so is `LEN`
/// of a parenthesized literal, of `MOVE` of one and of `CONCAT` of two,
/// nested or not.
#[test]
fn lower_statement_when_len_of_literal_then_constant_length() {
    let outcome = lower(
        "PROGRAM main VAR n : DINT; END_VAR
            n := LEN('Hello');
            n := LEN('');
            n := LEN(\"$00E9t$\"\");
            n := LEN(('abc'));
            n := LEN(MOVE('abcd'));
            n := LEN(CONCAT('ab', 'cde'));
            n := LEN(CONCAT(CONCAT('a', 'bc'), 'def'));
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (const i32s 5)",
            "assign v0^.f0 (const i32s 0)",
            "assign v0^.f0 (const i32s 3)",
            "assign v0^.f0 (const i32s 3)",
            "assign v0^.f0 (const i32s 4)",
            "assign v0^.f0 (const i32s 5)",
            "assign v0^.f0 (const i32s 6)",
        ]
    );
}

/// REQ-LOW-lowering-205: `LEN` of a constant string, declared or one the
/// program never writes, is a constant of its initial value's length, as
/// much of it as the string's capacity holds. A global reached through its
/// `VAR_EXTERNAL` is one too, and so is a `CONCAT` of a constant and a
/// literal.
#[test]
fn lower_statement_when_len_of_constant_variable_then_constant_length() {
    let outcome = lower(
        "CONFIGURATION config
            VAR_GLOBAL CONSTANT g : WSTRING := \"héllo\"; END_VAR
            RESOURCE res ON PLC
              TASK t (INTERVAL := T#100ms, PRIORITY := 1);
              PROGRAM p1 WITH t : main;
            END_RESOURCE
         END_CONFIGURATION
         PROGRAM main
         VAR_EXTERNAL CONSTANT g : WSTRING; END_VAR
         VAR CONSTANT declared : STRING := 'Hello'; capped : STRING[2] := 'Hello'; END_VAR
         VAR inferred : STRING[20] := 'abc'; n : DINT; END_VAR
            n := LEN(declared);
            n := LEN(capped);
            n := LEN(inferred);
            n := LEN(g);
            n := LEN(CONCAT(inferred, '!'));
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f3 (const i32s 5)",
            "assign v0^.f3 (const i32s 2)",
            "assign v0^.f3 (const i32s 3)",
            "assign v0^.f3 (const i32s 5)",
            "assign v0^.f3 (const i32s 4)",
        ]
    );
}

/// REQ-LOW-lowering-205: `LEN` of a string a run can change is the call
/// that reads its length when it runs: a variable the program writes, one
/// without an initial value, an element of an array of strings, and the
/// string another string function yields, or `CONCAT` yields of one such
/// string.
#[test]
fn lower_statement_when_len_of_changing_string_then_call() {
    let outcome = lower(
        "PROGRAM main
         VAR written : STRING[5] := 'abc'; unset : STRING[5];
             a : ARRAY[1..2] OF STRING[3] := ['ab', 'cd']; n : DINT; END_VAR
            written := 'x';
            n := LEN(written);
            n := LEN(unset);
            n := LEN(a[1]);
            n := LEN(LEFT('abc', 2));
            n := LEN(CONCAT('ab', written));
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-str v0^.f0 (literal string[1] \"x\")",
            "assign v0^.f3 (call len_string i32s (read string[5] v0^.f0))",
            "assign v0^.f3 (call len_string i32s (read string[5] v0^.f1))",
            "assign v0^.f3 (call len_string i32s (read string[3] v0^.f2[(const i32s 1)]))",
            "assign v0^.f3 (call len_string i32s (call left_string string[3] (literal string[3] \"abc\") (const i32s 2)))",
            "assign v0^.f3 (call len_string i32s (call concat_string string[7] (literal string[2] \"ab\") (read string[5] v0^.f0)))",
        ]
    );
}

/// REQ-LOW-lowering-205: a constant string of a function and of a function
/// block, read in its body, has the length of its initial value.
#[test]
fn lower_statement_when_len_of_constant_in_function_or_block_then_constant_length() {
    let source = "FUNCTION f : INT VAR CONSTANT s : STRING := 'abcd'; END_VAR
            f := LEN(s);
         END_FUNCTION
         FUNCTION_BLOCK fb VAR_OUTPUT n : INT; END_VAR VAR CONSTANT s : STRING := 'abcde'; END_VAR
            n := LEN(s);
         END_FUNCTION_BLOCK
         PROGRAM main VAR i : fb; n : INT; END_VAR
            n := f();
            i();
         END_PROGRAM";
    let function = lower(source, "f");
    let block = lower(source, "fb");
    assert_eq!(function.statements, ["assign v0 (const i32s 4)"]);
    assert_eq!(block.statements, ["assign v0^.f0 (const i32s 5)"]);
}

/// REQ-LOW-lowering-205: a constant member of a function block that an
/// instance initializer sets holds another value in that instance, so `LEN`
/// of it reads its length when it runs.
#[test]
fn lower_statement_when_len_of_constant_an_initializer_sets_then_call() {
    let outcome = lower(
        "FUNCTION_BLOCK fb VAR_OUTPUT n : INT; END_VAR VAR CONSTANT s : STRING := 'abcde'; END_VAR
            n := LEN(s);
         END_FUNCTION_BLOCK
         PROGRAM main VAR i : fb := (s := 'xy'); END_VAR
            i();
         END_PROGRAM",
        "fb",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f0 (truncate 16 (call len_string i32s (read string[254] v0^.f1)))"]
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

/// REQ-LOW-lowering-188: the string a reference refers to, and an element
/// of the array of strings one refers to, is read and stored through the
/// `Deref`, and `REF(s)^` is a read of `s`.
#[test]
fn lower_statement_when_string_through_reference_then_place_through_deref() {
    let outcome = lower(
        "PROGRAM main VAR s : STRING; t : STRING; rs : REF_TO STRING;
             rsa : REF_TO ARRAY[1..2] OF STRING; END_VAR
            t := rs^;
            rs^ := 'ab';
            t := rsa^[2];
            rsa^[2] := t;
            t := REF(s)^;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-str v0^.f1 (read string[254] v0^.f2^)",
            "assign-str v0^.f2^ (literal string[2] \"ab\")",
            "assign-str v0^.f1 (read string[254] v0^.f3^[(const i32s 2)])",
            "assign-str v0^.f3^[(const i32s 2)] (read string[254] v0^.f1)",
            "assign-str v0^.f1 (read string[254] v0^.f0)",
        ]
    );
}
