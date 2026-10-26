//! End-to-end tests for a time or date stored in a variable of its long
//! type, which the analyzer records as a conversion: a date after 2038,
//! stored unsigned, keeps its value rather than being sign-extended, however
//! the value is written -- a variable, a literal, an element or a field, a
//! call, an operator expression or the function form of the operator.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::{
    assert_run_with, date, datetime, time, Duration, FromValue, PrimitiveDateTime, Time,
};

/// Asserts that `result`, of `result_type`, holds `expected` after
/// `result := <value>` in a program declaring a short temporal variable of
/// each family, every date after 2038 and every `TIME` negative.
fn assert_stored<T: FromValue>(result_type: &str, value: &str, expected: T) {
    let program = format!(
        "TYPE Stamp : STRUCT moment : DATE_AND_TIME; END_STRUCT; END_TYPE
         FUNCTION_BLOCK Keep
         VAR_INPUT x : LDATE; END_VAR
         VAR_OUTPUT y : LDATE; END_VAR
           y := x;
         END_FUNCTION_BLOCK
         FUNCTION late : DATE_AND_TIME
           late := DT#2100-01-01-12:00:00;
         END_FUNCTION
         FUNCTION echo : LDATE_AND_TIME
         VAR_INPUT v : LDATE_AND_TIME; END_VAR
           echo := v;
         END_FUNCTION
         PROGRAM main
         VAR result : {result_type}; keep : Keep; keep_literal : Keep; stamp : Stamp;
             d : DATE := D#2100-01-01; dt1 : DT := DT#2100-01-01-12:00:00;
             dt2 : DT := DT#2100-01-01-12:00:05;
             stamps : ARRAY[1..2] OF DT := [DT#2100-01-01-12:00:00, DT#2100-01-01-12:00:00];
             ldt1 : LDT := LDT#2100-01-01-13:00:00;
             t : TIME := T#-5s; h : TIME := T#1h; tod1 : TOD := TOD#23:00:00; END_VAR
           keep(x := d);
           keep_literal(x := D#2100-01-01);
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
#[case::parenthesized("(dt1)", datetime!(2100-01-01 12:00))]
#[case::array_element("stamps[1]", datetime!(2100-01-01 12:00))]
#[case::structure_field("stamp.moment", datetime!(2100-01-01 12:00))]
#[case::user_function("late()", datetime!(2100-01-01 12:00))]
#[case::long_argument("echo(dt1)", datetime!(2100-01-01 12:00))]
#[case::operator_add("dt1 + h", datetime!(2100-01-01 13:00))]
#[case::typed_add("ADD_DT_TIME(dt1, h)", datetime!(2100-01-01 13:00))]
#[case::function_form_add("ADD(dt1, h)", datetime!(2100-01-01 13:00))]
#[case::operator_sub("dt1 - h", datetime!(2100-01-01 11:00))]
#[case::typed_sub("SUB_DT_TIME(dt1, h)", datetime!(2100-01-01 11:00))]
#[case::function_form_sub("SUB(dt1, h)", datetime!(2100-01-01 11:00))]
#[case::concat("CONCAT_DATE_TOD(d, tod1)", datetime!(2100-01-01 23:00))]
fn end_to_end_when_date_and_time_after_2038_written_any_way_stored_in_ldt_then_keeps_value(
    #[case] value: &str,
    #[case] expected: PrimitiveDateTime,
) {
    assert_stored("LDT", value, expected);
}

#[spec_test(REQ_IC_codegen_023)]
#[rstest]
#[case::literal("D#2100-01-01")]
#[case::literal_input("keep_literal.y")]
#[case::date_of("DT_TO_DATE(dt1)")]
fn end_to_end_when_date_after_2038_written_any_way_stored_in_ldate_then_keeps_value(
    #[case] value: &str,
) {
    assert_stored("LDATE", value, date!(2100 - 01 - 01));
}

#[spec_test(REQ_IC_codegen_023)]
#[rstest]
#[case::literal("TOD#23:00:00", time!(23:00))]
#[case::time_of_day_of("DT_TO_TOD(dt1)", time!(12:00))]
#[case::operator_sub("tod1 - h", time!(22:00))]
#[case::typed_sub("SUB_TOD_TIME(tod1, h)", time!(22:00))]
#[case::function_form_sub("SUB(tod1, h)", time!(22:00))]
fn end_to_end_when_time_of_day_written_any_way_stored_in_ltod_then_keeps_value(
    #[case] value: &str,
    #[case] expected: Time,
) {
    assert_stored("LTOD", value, expected);
}

#[spec_test(REQ_IC_codegen_023)]
#[rstest]
#[case::literal("T#-5s", Duration::seconds(-5))]
#[case::operator_add("t + t", Duration::seconds(-10))]
#[case::typed_add("ADD_TIME(t, t)", Duration::seconds(-10))]
#[case::function_form_add("ADD(t, t)", Duration::seconds(-10))]
#[case::operator_sub_dates("dt2 - dt1", Duration::seconds(5))]
#[case::function_form_sub_dates("SUB(dt2, dt1)", Duration::seconds(5))]
#[case::operator_mul("t * 2", Duration::seconds(-10))]
#[case::function_form_mul("MUL(t, 2)", Duration::seconds(-10))]
fn end_to_end_when_negative_time_written_any_way_stored_in_ltime_then_keeps_value(
    #[case] value: &str,
    #[case] expected: Duration,
) {
    assert_stored("LTIME", value, expected);
}

/// A short date compared with a long one is widened before the comparison.
#[spec_test(REQ_IC_codegen_023)]
#[rstest]
#[case::greater("ldt1 > dt1")]
#[case::equal_operator("ldt1 = dt1 + h")]
#[case::equal_function_form("ldt1 = ADD(dt1, h)")]
fn end_to_end_when_ldt_compared_with_date_and_time_after_2038_then_widened(#[case] value: &str) {
    assert_stored("BOOL", value, 1);
}
