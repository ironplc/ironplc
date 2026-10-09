//! The type of a field of a structure, wherever the structure is: a
//! variable, an element of an array variable, or an element of an array that
//! is itself a field.

use super::*;
use spec_test_macro::spec_test;

/// The type of `value` assigned in a program declaring structures nested in
/// arrays and fields of each kind the tests read.
fn member_type(value: &str) -> Option<String> {
    let program = format!(
        "TYPE Small : INT (-100..100); END_TYPE
         TYPE Inner : STRUCT v : LINT; END_STRUCT; END_TYPE
         TYPE Item : STRUCT a : DINT; r : Small; inner : ARRAY[1..2] OF Inner; END_STRUCT; END_TYPE
         TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; one : Item; END_STRUCT; END_TYPE
         PROGRAM main
         VAR x : LINT; h : Holder; it : Item; arr : ARRAY[1..2] OF Item; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let resolved = run_pass(&program);
    type_name_upper(&collect_assignment_types(&resolved)[0])
}

#[spec_test(REQ_IC_analyzer_095)]
#[rstest]
#[case::field_of_variable("it.a", "DINT")]
#[case::field_of_field("h.one.a", "DINT")]
#[case::field_of_array_element("arr[1].a", "DINT")]
#[case::field_of_array_field_element("h.items[1].a", "DINT")]
#[case::field_through_two_array_fields("h.items[1].inner[2].v", "LINT")]
#[case::subrange_field("it.r", "INT")]
#[case::subrange_field_of_array_field_element("h.items[2].r", "INT")]
#[case::arithmetic_on_fields("h.items[1].a + h.items[2].a", "DINT")]
fn apply_when_field_of_structure_then_has_declared_type(
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(member_type(value), Some(expected.to_string()));
}
