//! End-to-end tests for a time or date stored in a variable of its long
//! type, which the analyzer records as a conversion: a date after 2038,
//! stored unsigned, keeps its value rather than being sign-extended, and a
//! negative time stays negative, whatever names the value
//! ([#1877](https://github.com/ironplc/ironplc/issues/1877)). A bit string
//! literal, which has a type of its own as a time or date literal does, is
//! tested with them.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::{
    assert_run_with, date, datetime, time, Duration, FromValue, PrimitiveDateTime, Time,
};

/// Asserts that `result`, of `result_type`, holds `expected` after
/// `result := <value>` in a program declaring a short temporal variable of
/// each family, a date and time after 2038 held in an array and a structure
/// and returned by a function, and a function of a long parameter.
fn assert_stored<T: FromValue>(result_type: &str, value: &str, expected: T) {
    let program = format!(
        "TYPE
           Stamp : STRUCT moment : DATE_AND_TIME; END_STRUCT;
         END_TYPE
         FUNCTION late_dt : DATE_AND_TIME
           late_dt := DT#2100-01-01-12:00:00;
         END_FUNCTION
         FUNCTION echo_ldt : LDATE_AND_TIME
         VAR_INPUT v : LDATE_AND_TIME; END_VAR
           echo_ldt := v;
         END_FUNCTION
         FUNCTION_BLOCK Keep
         VAR_INPUT x : LDATE; END_VAR
         VAR_OUTPUT y : LDATE; END_VAR
           y := x;
         END_FUNCTION_BLOCK
         PROGRAM main
         VAR result : {result_type}; keep : Keep;
             d : DATE := D#2100-01-01; dt1 : DT := DT#2100-01-01-12:00:00;
             t : TIME := T#-5s; tod1 : TOD := TOD#23:00:00; hour : TIME := T#1h;
             stamps : ARRAY[1..2] OF DATE_AND_TIME := [DT#2100-01-01-12:00:00, DT#2100-01-01-12:00:00];
             stamp : Stamp; END_VAR
           keep(x := d);
           stamp.moment := dt1;
           result := {value};
         END_PROGRAM"
    );
    assert_run_with(
        &program,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[("result", expected)],
    );
}

#[spec_test(REQ_IC_codegen_014)]
#[test]
fn end_to_end_when_date_after_2038_stored_in_ldate_then_keeps_value() {
    assert_stored("LDATE", "d", date!(2100 - 01 - 01));
}

#[spec_test(REQ_IC_codegen_014)]
#[test]
fn end_to_end_when_date_and_time_after_2038_stored_in_ldt_then_keeps_value() {
    assert_stored("LDT", "dt1", datetime!(2100-01-01 12:00));
}

#[spec_test(REQ_IC_codegen_014)]
#[test]
fn end_to_end_when_date_after_2038_passed_to_ldate_input_then_keeps_value() {
    assert_stored("LDATE", "keep.y", date!(2100 - 01 - 01));
}

#[spec_test(REQ_IC_codegen_014)]
#[test]
fn end_to_end_when_negative_time_stored_in_ltime_then_keeps_value() {
    assert_stored("LTIME", "t", Duration::seconds(-5));
}

#[spec_test(REQ_IC_codegen_014)]
#[test]
fn end_to_end_when_time_of_day_stored_in_ltod_then_keeps_value() {
    assert_stored("LTOD", "tod1", time!(23:00));
}

#[spec_test(REQ_IC_codegen_023)]
#[rstest]
#[case::literal("DT#2100-01-01-12:00:00", datetime!(2100-01-01 12:00))]
#[case::function_form_add("ADD(dt1, hour)", datetime!(2100-01-01 13:00))]
#[case::function_form_sub("SUB(dt1, hour)", datetime!(2100-01-01 11:00))]
#[case::operator("dt1 + hour", datetime!(2100-01-01 13:00))]
#[case::typed_call("ADD_DT_TIME(dt1, hour)", datetime!(2100-01-01 13:00))]
#[case::concat_date_tod("CONCAT_DATE_TOD(d, tod1)", datetime!(2100-01-01 23:00))]
#[case::parenthesized("(dt1)", datetime!(2100-01-01 12:00))]
#[case::array_element("stamps[1]", datetime!(2100-01-01 12:00))]
#[case::structure_field("stamp.moment", datetime!(2100-01-01 12:00))]
#[case::user_function_result("late_dt()", datetime!(2100-01-01 12:00))]
#[case::user_function_argument("echo_ldt(dt1)", datetime!(2100-01-01 12:00))]
fn end_to_end_when_ldt_stores_date_and_time_after_2038_by_any_spelling_then_keeps_value(
    #[case] value: &str,
    #[case] expected: PrimitiveDateTime,
) {
    assert_stored("LDT", value, expected);
}

#[spec_test(REQ_IC_codegen_023)]
#[test]
fn end_to_end_when_ldate_stores_date_literal_after_2038_then_keeps_value() {
    assert_stored("LDATE", "D#2100-01-01", date!(2100 - 01 - 01));
}

/// A bit string literal has a type of its own as a time or date literal
/// does, so a `DWORD` literal with its top bit set reaches an `LWORD`
/// zero-extended.
#[spec_test(REQ_IC_codegen_023)]
#[test]
fn end_to_end_when_lword_stores_dword_literal_with_top_bit_set_then_keeps_value() {
    assert_stored("LWORD", "DWORD#16#FFFF_FFFF", 4_294_967_295_u64);
}

#[spec_test(REQ_IC_codegen_023)]
#[rstest]
#[case::literal("T#-5s", Duration::seconds(-5))]
#[case::function_form("ADD(t, t)", Duration::seconds(-10))]
#[case::function_form_of_three_inputs("ADD(t, t, t)", Duration::seconds(-15))]
#[case::operator("t + t", Duration::seconds(-10))]
#[case::typed_call("ADD_TIME(t, t)", Duration::seconds(-10))]
fn end_to_end_when_ltime_stores_negative_time_by_any_spelling_then_keeps_value(
    #[case] value: &str,
    #[case] expected: Duration,
) {
    assert_stored("LTIME", value, expected);
}

#[spec_test(REQ_IC_codegen_023)]
#[rstest]
#[case::literal("TOD#23:00:00", time!(23:00))]
#[case::function_form("ADD(tod1, t)", time!(22:59:55))]
#[case::operator("tod1 + t", time!(22:59:55))]
#[case::typed_call("ADD_TOD_TIME(tod1, t)", time!(22:59:55))]
#[case::dt_to_tod("DT_TO_TOD(dt1)", time!(12:00))]
fn end_to_end_when_ltod_stores_time_of_day_by_any_spelling_then_keeps_value(
    #[case] value: &str,
    #[case] expected: Time,
) {
    assert_stored("LTOD", value, expected);
}
