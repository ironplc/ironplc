//! Tests of lowering arithmetic on times and dates.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower;

/// A typed time function is arithmetic at the signed integer of its width,
/// on the milliseconds of a `TIME` or `TIME_OF_DAY` and the seconds of a
/// `DATE` or `DATE_AND_TIME`. An unsigned operand or result changes only its
/// signedness.
#[test]
fn lower_statement_when_typed_time_function_then_arithmetic_of_its_units() {
    let outcome = lower(
        "PROGRAM main VAR t : TIME; t2 : TIME; c : TOD; c2 : TOD; s : DT; s2 : DT;
             d : DATE; lt : LTIME; ls : LDT; END_VAR
            t := ADD_TIME(t, t2);
            c := ADD_TOD_TIME(c, t);
            t := SUB_TOD_TOD(c, c2);
            s := ADD_DT_TIME(s, t);
            t := SUB_DT_DT(s, s2);
            s := CONCAT_DATE_TOD(d, c);
            ls := SUB_LDT_LTIME(ls, lt);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (add i32s (read i32s v0^.f0) (read i32s v0^.f1))",
            "assign v0^.f2 (convert i32u (add i32s (convert i32s (read i32u v0^.f2)) \
             (read i32s v0^.f0)))",
            "assign v0^.f0 (sub i32s (convert i32s (read i32u v0^.f2)) \
             (convert i32s (read i32u v0^.f3)))",
            "assign v0^.f4 (convert i32u (add i32s (convert i32s (read i32u v0^.f4)) \
             (div i32s (read i32s v0^.f0) (const i32s 1000))))",
            "assign v0^.f0 (mul i32s (sub i32s (convert i32s (read i32u v0^.f4)) \
             (convert i32s (read i32u v0^.f5))) (const i32s 1000))",
            "assign v0^.f4 (convert i32u (add i32s (convert i32s (read i32u v0^.f6)) \
             (div i32s (convert i32s (read i32u v0^.f2)) (const i32s 1000))))",
            "assign v0^.f8 (convert i64u (sub i64s (convert i64s (read i64u v0^.f8)) \
             (div i64s (read i64s v0^.f7) (const i64s 1000))))",
        ]
    );
}

/// REQ-LOW-lowering-166: an operator on times and dates, and its function
/// form, lower to the expression of the typed function they resolve to. A
/// fold of three times is the typed function folded from the left.
#[test]
fn lower_statement_when_operator_on_times_then_same_as_typed_function() {
    let forms = [
        ["t := ADD_TIME(t, t2);", "t := t + t2;", "t := ADD(t, t2);"],
        ["c := SUB_TOD_TIME(c, t);", "c := c - t;", "c := SUB(c, t);"],
        [
            "t := SUB_TOD_TOD(c, c2);",
            "t := c - c2;",
            "t := SUB(c, c2);",
        ],
        ["s := ADD_DT_TIME(s, t);", "s := s + t;", "s := ADD(s, t);"],
        [
            "t := SUB_DATE_DATE(d, d2);",
            "t := d - d2;",
            "t := SUB(d, d2);",
        ],
        ["t := MUL_TIME(t, r);", "t := t * r;", "t := MUL(t, r);"],
        ["t := DIV_TIME(t, 2);", "t := t / 2;", "t := DIV(t, 2);"],
        [
            "lt := ADD_LTIME(lt, t);",
            "lt := lt + t;",
            "lt := ADD(lt, t);",
        ],
        [
            "lt := MUL_LTIME(lt, i);",
            "lt := lt * i;",
            "lt := MUL(lt, i);",
        ],
        [
            "t := ADD_TIME(ADD_TIME(t, t2), t);",
            "t := t + t2 + t;",
            "t := ADD(t, t2, t);",
        ],
    ];
    for [typed, operator, form] in forms {
        let outcome = lower(
            &format!(
                "PROGRAM main VAR t : TIME; t2 : TIME; c : TOD; c2 : TOD; s : DT;
                     d : DATE; d2 : DATE; lt : LTIME; r : REAL; i : DINT; END_VAR
                    {typed}
                    {operator}
                    {form}
                 END_PROGRAM"
            ),
            "main",
        );
        assert_ne!(outcome.statements[0], "unsupported", "{typed}");
        assert_eq!(outcome.statements[0], outcome.statements[1], "{operator}");
        assert_eq!(outcome.statements[0], outcome.statements[2], "{form}");
    }
}

/// `MUL_TIME` and `DIV_TIME` scale a duration by an integer of its width
/// directly, and by a real converted to the real's type and back. A `REAL`
/// scaling an `LTIME` arrives converted to `LREAL`.
#[test]
fn lower_statement_when_duration_scaled_then_by_integer_or_through_real() {
    let outcome = lower(
        "PROGRAM main VAR t : TIME; lt : LTIME; d : DINT; u : UDINT; r : REAL; lr : LREAL; END_VAR
            t := MUL_TIME(t, d);
            t := DIV_TIME(t, u);
            t := MUL_TIME(t, r);
            t := DIV_TIME(t, lr);
            lt := MUL_LTIME(lt, r);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (mul i32s (read i32s v0^.f0) (read i32s v0^.f2))",
            "assign v0^.f0 (div i32s (read i32s v0^.f0) (convert i32s (read i32u v0^.f3)))",
            "assign v0^.f0 (convert i32s (mul f32 (convert f32 (read i32s v0^.f0)) \
             (read f32 v0^.f4)))",
            "assign v0^.f0 (convert i32s (div f64 (convert f64 (read i32s v0^.f0)) \
             (read f64 v0^.f5)))",
            "assign v0^.f1 (convert i64s (mul f64 (convert f64 (read i64s v0^.f1)) \
             (convert f64 (read f32 v0^.f4))))",
        ]
    );
}

/// REQ-LOW-lowering-165: `DT_TO_DATE` rounds its date and time down to the
/// day reading it once, and `DT_TO_TOD` takes the seconds since midnight in
/// milliseconds.
#[test]
fn lower_statement_when_date_and_time_split_then_reads_it_once() {
    let outcome = lower(
        "PROGRAM main VAR s : DT; d : DATE; c : TOD; END_VAR
            d := DT_TO_DATE(s);
            c := DATE_AND_TIME_TO_TIME_OF_DAY(s);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f1 (mul i32u (div i32u (read i32u v0^.f0) (const i32u 86400)) \
             (const i32u 86400))",
            "assign v0^.f2 (mul i32u (mod i32u (read i32u v0^.f0) (const i32u 86400)) \
             (const i32u 1000))",
        ]
    );
}

/// A time or date literal is the count it is stored as (ADR-0025): the
/// milliseconds of a duration or a time of day, and the seconds since
/// 1970-01-01 of a date or a date and time.
#[test]
fn lower_statement_when_time_or_date_literal_then_constant_of_its_count() {
    let outcome = lower(
        "PROGRAM main VAR t : TIME; c : TOD; d : DATE; s : DT; END_VAR
            t := T#1s500ms;
            c := TOD#01:00:00.25;
            d := D#1970-01-02;
            s := DT#1970-01-02-00:01:00;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (const i32s 1500)",
            "assign v0^.f1 (const i32u 3600250)",
            "assign v0^.f2 (const i32u 86400)",
            "assign v0^.f3 (const i32u 86460)",
        ]
    );
}

/// A duration is negated at the signed integer it is stored as (ADR-0025):
/// the milliseconds of a `TIME` at 32 bits and of an `LTIME` at 64.
#[test]
fn lower_statement_when_duration_negated_then_negation_of_its_count() {
    let outcome = lower(
        "PROGRAM main VAR t : TIME; lt : LTIME; END_VAR
            t := -t;
            lt := -lt;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (neg i32s (read i32s v0^.f0))",
            "assign v0^.f1 (neg i64s (read i64s v0^.f1))",
        ]
    );
}
