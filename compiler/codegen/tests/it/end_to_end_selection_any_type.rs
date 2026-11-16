//! End-to-end tests for `MOVE`, `SEL` and `MUX` of every type they are
//! lowered at besides a number: a `BOOL`, a bit string, a time or date, an
//! enumeration and a subrange, and `MOVE` of a string, a structure and an
//! array. Each gives the value of the input it copies or selects, and a
//! short input selected beside a long one is widened by its signedness.
//! And for `MIN`, `MAX` and `LIMIT` of the elementary types besides a
//! number: a `BOOL`, a bit string, a time or date, and a string, each of
//! which gives the input its order selects.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::{
    assert_run_with, date, datetime, time, Date, Duration, FromValue, PrimitiveDateTime, Snapshot,
    Time,
};

/// Asserts that `result`, of `result_type`, holds `expected` after
/// `result := <value>` in a program declaring two variables of each type,
/// the first input `g` selects (`FALSE`) holding the earlier value.
fn assert_selected<T: FromValue>(result_type: &str, value: &str, expected: T) {
    let program = format!(
        "TYPE Color : (Red, Green, Blue); END_TYPE
         TYPE Digit : DINT (0..9); END_TYPE
         PROGRAM main
         VAR result : {result_type}; g : BOOL := TRUE; k : DINT := 1; low : DINT := -1;
             high : DINT := 9;
             b0 : BOOL := FALSE; b1 : BOOL := TRUE;
             w0 : WORD := WORD#16#00FF; w1 : WORD := WORD#16#FF00;
             lw0 : LWORD := LWORD#16#FFFF_FFFF_0000_0000;
             t0 : TIME := T#2s; t1 : TIME := T#-5s; lt0 : LTIME := LTIME#3s;
             d0 : DATE := D#2020-01-01; d1 : DATE := D#2100-01-01;
             ld0 : LDATE := LDATE#2101-01-01;
             tod0 : TOD := TOD#01:00:00; tod1 : TOD := TOD#23:00:00;
             dt0 : DT := DT#2020-01-01-00:00:00; dt1 : DT := DT#2100-01-01-12:00:00;
             c0 : Color := Red; c1 : Color := Blue;
             n0 : Digit := 3; n1 : Digit := 7; END_VAR
           result := {value};
         END_PROGRAM"
    );
    assert_run_with(
        &program,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[("result", expected)],
    );
}

#[spec_test(REQ_IC_codegen_024)]
#[rstest]
#[case::sel_true("SEL(g, b0, b1)", 1)]
#[case::sel_false("SEL(NOT g, b0, b1)", 0)]
#[case::sel_false_selected("SEL(g, b1, b0)", 0)]
#[case::sel_literals("SEL(g, FALSE, TRUE)", 1)]
#[case::mux("MUX(k, b0, b1, b0)", 1)]
#[case::mux_below_range("MUX(low, b1, b0)", 1)]
#[case::mux_above_range("MUX(high, b0, b1)", 1)]
#[case::mux_literals("MUX(0, TRUE, FALSE)", 1)]
#[case::move_bool("MOVE(b1)", 1)]
fn end_to_end_when_bool_selected_or_moved_then_stores_input(
    #[case] value: &str,
    #[case] expected: i32,
) {
    assert_selected("BOOL", value, expected);
}

#[spec_test(REQ_IC_codegen_024)]
#[rstest]
#[case::sel_word("WORD", "SEL(g, w0, w1)", 0xFF00)]
#[case::mux_word("WORD", "MUX(k, w0, w1)", 0xFF00)]
#[case::move_word("WORD", "MOVE(w0)", 0x00FF)]
#[case::sel_word_beside_lword("LWORD", "SEL(g, lw0, w1)", 0xFF00)]
#[case::sel_enumeration("Color", "SEL(g, c0, c1)", 2)]
#[case::sel_enumeration_values("Color", "SEL(g, Red, Green)", 1)]
#[case::mux_enumeration("Color", "MUX(k, c0, c1)", 2)]
#[case::move_enumeration("Color", "MOVE(c1)", 2)]
#[case::sel_subrange("Digit", "SEL(g, n0, n1)", 7)]
#[case::mux_subrange("Digit", "MUX(0, n0, n1)", 3)]
#[case::move_subrange("Digit", "MOVE(n1)", 7)]
fn end_to_end_when_bit_string_enumeration_or_subrange_selected_then_stores_input(
    #[case] result_type: &str,
    #[case] value: &str,
    #[case] expected: i64,
) {
    assert_selected(result_type, value, expected);
}

#[spec_test(REQ_IC_codegen_024)]
#[rstest]
#[case::sel_time("TIME", "SEL(g, t0, t1)")]
#[case::mux_time("TIME", "MUX(k, t0, t1)")]
#[case::move_time("TIME", "MOVE(t1)")]
#[case::sel_time_beside_ltime("LTIME", "SEL(g, lt0, t1)")]
#[case::move_time_into_ltime("LTIME", "MOVE(t1)")]
fn end_to_end_when_negative_time_selected_or_moved_then_keeps_value(
    #[case] result_type: &str,
    #[case] value: &str,
) {
    assert_selected(result_type, value, Duration::seconds(-5));
}

#[spec_test(REQ_IC_codegen_024)]
#[rstest]
#[case::sel_date("DATE", "SEL(g, d0, d1)", date!(2100 - 01 - 01))]
#[case::mux_date("DATE", "MUX(k, d0, d1)", date!(2100 - 01 - 01))]
#[case::move_date("DATE", "MOVE(d1)", date!(2100 - 01 - 01))]
#[case::sel_date_beside_ldate("LDATE", "SEL(g, ld0, d1)", date!(2100 - 01 - 01))]
#[case::mux_date_beside_ldate("LDATE", "MUX(0, ld0, d1)", date!(2101 - 01 - 01))]
#[case::move_date_into_ldate("LDATE", "MOVE(d1)", date!(2100 - 01 - 01))]
fn end_to_end_when_date_after_2038_selected_or_moved_then_keeps_value(
    #[case] result_type: &str,
    #[case] value: &str,
    #[case] expected: Date,
) {
    assert_selected(result_type, value, expected);
}

#[spec_test(REQ_IC_codegen_024)]
#[rstest]
#[case::sel_time_of_day("TOD", "SEL(g, tod0, tod1)", time!(23:00))]
#[case::mux_time_of_day("TOD", "MUX(k, tod0, tod1)", time!(23:00))]
#[case::move_time_of_day_into_ltod("LTOD", "MOVE(tod1)", time!(23:00))]
fn end_to_end_when_time_of_day_selected_or_moved_then_keeps_value(
    #[case] result_type: &str,
    #[case] value: &str,
    #[case] expected: Time,
) {
    assert_selected(result_type, value, expected);
}

#[spec_test(REQ_IC_codegen_024)]
#[rstest]
#[case::sel_date_and_time("DT", "SEL(g, dt0, dt1)")]
#[case::mux_date_and_time("DT", "MUX(k, dt0, dt1)")]
#[case::move_date_and_time("DT", "MOVE(dt1)")]
#[case::sel_into_ldt("LDT", "SEL(g, dt0, dt1)")]
#[case::move_into_ldt("LDT", "MOVE(dt1)")]
fn end_to_end_when_date_and_time_after_2038_selected_or_moved_then_keeps_value(
    #[case] result_type: &str,
    #[case] value: &str,
) {
    assert_selected(result_type, value, datetime!(2100-01-01 12:00));
}

/// `MOVE` of a string, a structure or an array copies it, assigned and as
/// an argument of a string function.
#[spec_test(REQ_IC_codegen_024)]
#[test]
fn end_to_end_when_string_structure_or_array_moved_then_copied() {
    let source = r#"
TYPE Point : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
PROGRAM main
  VAR
    s0 : STRING := 'hello'; s1 : STRING; short : STRING[3]; joined : STRING;
    ws0 : WSTRING := "wide"; ws1 : WSTRING;
    p0 : Point := (x := 1, y := 2); p1 : Point; px : DINT; py : DINT;
    a0 : ARRAY[1..2] OF DINT := [3, 4]; a1 : ARRAY[1..2] OF DINT; first : DINT; second : DINT;
  END_VAR
  s1 := MOVE(s0);
  short := MOVE(s0);
  joined := CONCAT(MOVE(s0), '!');
  ws1 := MOVE(ws0);
  p1 := MOVE(p0);
  px := p1.x;
  py := p1.y;
  a1 := MOVE(a0);
  first := a1[1];
  second := a1[2];
END_PROGRAM
"#;
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read("s1"), "hello");
    assert_eq!(snapshot.read("short"), "hel");
    assert_eq!(snapshot.read("joined"), "hello!");
    assert_eq!(snapshot.read("ws1"), "wide");
    for (name, expected) in [("px", 1), ("py", 2), ("first", 3), ("second", 4)] {
        assert_eq!(snapshot.read_as::<i32>(name), expected, "{name}");
    }
}

/// `SEL` and `MUX` of strings, structures and arrays whose inputs are
/// literals or variables store the input they select: a selector of `MUX`
/// out of range is clamped, and a string is truncated to its target.
#[spec_test(REQ_IC_codegen_025)]
#[test]
fn end_to_end_when_string_structure_or_array_variables_selected_then_stores_selected_input() {
    let source = r#"
TYPE Point : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
TYPE Other : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
FUNCTION pick : STRING
  VAR_INPUT g : BOOL; a : STRING; b : STRING; END_VAR
  pick := SEL(g, a, b);
END_FUNCTION
PROGRAM main
  VAR
    g : BOOL := TRUE; k : DINT := 1; low : DINT := -3; high : DINT := 7;
    s0 : STRING := 'zero'; s1 : STRING := 'one'; s2 : STRING := 'two';
    ws0 : WSTRING := "wide0"; ws1 : WSTRING := "wide1";
    sel_s : STRING; mux_s : STRING; mux_low : STRING; mux_high : STRING; short : STRING[2];
    sel_ws : WSTRING; literal : STRING; picked : STRING;
    p0 : Point := (x := 1, y := 2); o1 : Other := (x := 3, y := 4); p : Point;
    px : DINT; py : DINT;
    a0 : ARRAY[1..2] OF DINT := [5, 6]; a1 : ARRAY[1..2] OF DINT := [7, 8];
    a : ARRAY[1..2] OF DINT; first : DINT; second : DINT;
  END_VAR
  sel_s := SEL(g, s0, s1);
  mux_s := MUX(k, s0, s1, s2);
  mux_low := MUX(low, s0, s1, s2);
  mux_high := MUX(high, s0, s1, s2);
  short := SEL(g, s0, s1);
  sel_ws := SEL(NOT g, ws0, ws1);
  literal := SEL(g, s0, 'literal');
  picked := pick(FALSE, s0, s1);
  p := SEL(g, p0, o1);
  px := p.x;
  py := p.y;
  a := MUX(0, a0, a1);
  first := a[1];
  second := a[2];
END_PROGRAM
"#;
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    for (name, expected) in [
        ("sel_s", "one"),
        ("mux_s", "one"),
        ("mux_low", "zero"),
        ("mux_high", "two"),
        ("short", "on"),
        ("sel_ws", "wide0"),
        ("literal", "literal"),
        ("picked", "zero"),
    ] {
        assert_eq!(snapshot.read(name), expected, "{name}");
    }
    for (name, expected) in [("px", 3), ("py", 4), ("first", 5), ("second", 6)] {
        assert_eq!(snapshot.read_as::<i32>(name), expected, "{name}");
    }
}

/// A selection of strings, structures or arrays whose inputs need
/// temporaries, an input with a call, a computed subscript or another
/// selection, is not supported yet: this backend gives no temporary storage
/// of its own.
#[spec_test(REQ_IC_codegen_025)]
#[rstest]
#[case::call("s := SEL(g, CONCAT(s, t), t);")]
#[case::computed_subscript("s := SEL(g, names[i], t);")]
#[case::nested_selection("s := SEL(g, s, MUX(i, s, t));")]
fn end_to_end_when_selection_inputs_need_temporaries_then_not_supported_yet(#[case] body: &str) {
    let source = format!(
        "PROGRAM main VAR g : BOOL; i : DINT := 1; names : ARRAY[1..2] OF STRING; s : STRING;
             t : STRING; END_VAR {body} END_PROGRAM"
    );
    let compiled = crate::common::try_parse_and_compile(&source, &CompilerOptions::default());
    let Err(diagnostic) = compiled else {
        panic!("{body} compiled");
    };
    assert_eq!(diagnostic.code, "P9999", "{body}");
}

/// The example of the issue that declared `MIN`, `MAX` and `LIMIT` on
/// `ANY_ELEMENTARY`: the least of two durations, and one limited to a range.
#[spec_test(REQ_IC_codegen_026)]
#[test]
fn end_to_end_when_min_and_limit_of_times_then_selects_time() {
    let program = "PROGRAM main
         VAR a : TIME := T#2s; b : TIME := T#500ms; lo : TIME; clamp : TIME; END_VAR
           lo := MIN(a, b);
           clamp := LIMIT(T#100ms, a, T#5s);
         END_PROGRAM";
    assert_run_with(
        program,
        &CompilerOptions::default(),
        &[
            ("lo", Duration::milliseconds(500)),
            ("clamp", Duration::seconds(2)),
        ],
    );
}

#[spec_test(REQ_IC_codegen_026)]
#[rstest]
#[case::min("MIN(b0, b1)", 0)]
#[case::max("MAX(b0, b1)", 1)]
#[case::max_literals("MAX(FALSE, TRUE)", 1)]
#[case::min_of_three("MIN(b1, b1, b0)", 0)]
#[case::max_of_three("MAX(b0, b0, b1)", 1)]
#[case::limit_within("LIMIT(b0, b1, b1)", 1)]
#[case::limit_below_mn("LIMIT(b1, b0, b1)", 1)]
#[case::limit_mn_above_mx("LIMIT(b1, b1, b0)", 0)]
fn end_to_end_when_min_max_or_limit_of_bools_then_selects_by_false_below_true(
    #[case] value: &str,
    #[case] expected: i32,
) {
    assert_selected("BOOL", value, expected);
}

#[spec_test(REQ_IC_codegen_026)]
#[rstest]
#[case::max_word("WORD", "MAX(w0, w1)", 0xFF00)]
#[case::min_word("WORD", "MIN(w0, w1)", 0x00FF)]
#[case::limit_word("WORD", "LIMIT(w0, WORD#16#FFFF, w1)", 0xFF00)]
#[case::min_word_beside_lword("LWORD", "MIN(lw0, w1)", 0xFF00)]
fn end_to_end_when_min_max_or_limit_of_bit_strings_then_selects_by_value(
    #[case] result_type: &str,
    #[case] value: &str,
    #[case] expected: i64,
) {
    assert_selected(result_type, value, expected);
}

#[spec_test(REQ_IC_codegen_026)]
#[rstest]
#[case::min_time("TIME", "MIN(t0, t1)", Duration::seconds(-5))]
#[case::max_time("TIME", "MAX(t1, t0)", Duration::seconds(2))]
#[case::max_of_three_times("TIME", "MAX(t1, t0, T#1s)", Duration::seconds(2))]
#[case::limit_time("TIME", "LIMIT(t1, T#10s, t0)", Duration::seconds(2))]
#[case::limit_time_mn_above_mx("TIME", "LIMIT(t0, t1, T#1s)", Duration::seconds(1))]
#[case::min_time_beside_ltime("LTIME", "MIN(lt0, t1)", Duration::seconds(-5))]
#[case::max_ltime("LTIME", "MAX(lt0, t0)", Duration::seconds(3))]
fn end_to_end_when_min_max_or_limit_of_times_then_selects_by_duration(
    #[case] result_type: &str,
    #[case] value: &str,
    #[case] expected: Duration,
) {
    assert_selected(result_type, value, expected);
}

#[spec_test(REQ_IC_codegen_026)]
#[rstest]
#[case::max_date("DATE", "MAX(d0, d1)", date!(2100 - 01 - 01))]
#[case::min_date("DATE", "MIN(d1, d0)", date!(2020 - 01 - 01))]
#[case::limit_date("DATE", "LIMIT(d0, d1, D#2050-01-01)", date!(2050 - 01 - 01))]
#[case::min_date_beside_ldate("LDATE", "MIN(ld0, d1)", date!(2100 - 01 - 01))]
fn end_to_end_when_min_max_or_limit_of_dates_then_selects_by_date(
    #[case] result_type: &str,
    #[case] value: &str,
    #[case] expected: Date,
) {
    assert_selected(result_type, value, expected);
}

#[spec_test(REQ_IC_codegen_026)]
#[rstest]
#[case::min_time_of_day("MIN(tod1, tod0)", time!(01:00))]
#[case::max_time_of_day("MAX(tod0, tod1)", time!(23:00))]
#[case::limit_time_of_day("LIMIT(tod0, tod1, TOD#12:00:00)", time!(12:00))]
fn end_to_end_when_min_max_or_limit_of_times_of_day_then_selects_by_time(
    #[case] value: &str,
    #[case] expected: Time,
) {
    assert_selected("TOD", value, expected);
}

#[spec_test(REQ_IC_codegen_026)]
#[rstest]
#[case::max_date_and_time("MAX(dt0, dt1)", datetime!(2100-01-01 12:00))]
#[case::min_date_and_time("MIN(dt1, dt0)", datetime!(2020-01-01 00:00))]
fn end_to_end_when_min_or_max_of_dates_and_times_then_selects_by_instant(
    #[case] value: &str,
    #[case] expected: PrimitiveDateTime,
) {
    assert_selected("DT", value, expected);
}

/// `MIN`, `MAX` and `LIMIT` of strings store the input their order selects:
/// of variables and literals, of wide strings, of three inputs, with `MN`
/// above `MX`, truncated to the target, and stored at one of the inputs.
#[spec_test(REQ_IC_codegen_026)]
#[test]
fn end_to_end_when_min_max_or_limit_of_strings_then_stores_input_by_order() {
    let source = r#"
PROGRAM main
  VAR
    a : STRING := 'pear'; b : STRING := 'apple'; c : STRING := 'fig';
    w0 : WSTRING := "wolf"; w1 : WSTRING := "bear";
    lowest : STRING; highest : STRING; least_of_three : STRING; limited : STRING;
    mn_above_mx : STRING; short : STRING[3]; wide : WSTRING;
    target : STRING := 'zeta'; prefix : STRING;
  END_VAR
  lowest := MIN('b', 'ab');
  highest := MAX(a, b);
  least_of_three := MIN('b', 'ab', 'c');
  limited := LIMIT(b, 'zebra', a);
  mn_above_mx := LIMIT(a, c, b);
  short := MAX(a, b);
  wide := MIN(w0, w1);
  target := MIN(target, c);
  prefix := MAX('ab', 'a');
END_PROGRAM
"#;
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    for (name, expected) in [
        ("lowest", "ab"),
        ("highest", "pear"),
        ("least_of_three", "ab"),
        ("limited", "pear"),
        ("mn_above_mx", "apple"),
        ("short", "pea"),
        ("wide", "bear"),
        ("target", "fig"),
        ("prefix", "ab"),
    ] {
        assert_eq!(snapshot.read(name), expected, "{name}");
    }
}

/// `MIN`, `MAX` and `LIMIT` of strings whose inputs need temporaries are not
/// supported yet, as a selection whose inputs need them is not.
#[spec_test(REQ_IC_codegen_026)]
#[rstest]
#[case::call("s := MIN(CONCAT(s, t), t);")]
#[case::computed_subscript("s := MAX(names[i], t);")]
#[case::nested("s := LIMIT(s, MIN(s, t), t);")]
fn end_to_end_when_min_max_or_limit_of_strings_need_temporaries_then_not_supported_yet(
    #[case] body: &str,
) {
    let source = format!(
        "PROGRAM main VAR i : DINT := 1; names : ARRAY[1..2] OF STRING; s : STRING;
             t : STRING; END_VAR {body} END_PROGRAM"
    );
    let compiled = crate::common::try_parse_and_compile(&source, &CompilerOptions::default());
    let code = compiled.err().map(|diagnostic| diagnostic.code);
    assert_eq!(code.as_deref(), Some("P9999"), "{body}");
}
