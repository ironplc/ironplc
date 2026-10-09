//! End-to-end tests for a time or date stored in a variable of its long
//! type, which the analyzer records as a conversion: a date after 2038,
//! stored unsigned, keeps its value rather than being sign-extended.

use ironplc_parser::options::{CompilerOptions, Dialect};
use spec_test_macro::spec_test;

use crate::common::{assert_run_with, date, datetime, time, Duration, FromValue};

/// Asserts that `result`, of `result_type`, holds `expected` after
/// `result := <value>` in a program declaring a short temporal variable of
/// each family.
fn assert_stored<T: FromValue>(result_type: &str, value: &str, expected: T) {
    let program = format!(
        "FUNCTION_BLOCK Keep
         VAR_INPUT x : LDATE; END_VAR
         VAR_OUTPUT y : LDATE; END_VAR
           y := x;
         END_FUNCTION_BLOCK
         PROGRAM main
         VAR result : {result_type}; keep : Keep;
             d : DATE := D#2100-01-01; dt1 : DT := DT#2100-01-01-12:00:00;
             t : TIME := T#-5s; tod1 : TOD := TOD#23:00:00; END_VAR
           keep(x := d);
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
