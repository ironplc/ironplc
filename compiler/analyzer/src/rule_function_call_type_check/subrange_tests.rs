//! Tests for values of a subrange type, which are operated at the
//! subrange's base type: an operation on one assigned to a narrower target is
//! a mismatch, and a subrange parameter takes a value of its base type.

use ironplc_problems::Problem;
use spec_test_macro::spec_test;

/// The subrange types the programs of these tests declare.
macro_rules! with_types {
    ($program:literal) => {
        concat!(
            "TYPE Small : INT (-100..100); Big : LINT (0..10000000000); END_TYPE
             FUNCTION f : DINT VAR_INPUT p : Small; END_VAR f := p; END_FUNCTION ",
            $program
        )
    };
}

rule_err!(
    #[spec_test(REQ_IC_analyzer_093)]
    apply_when_subrange_sum_assigned_to_narrower_target_then_assignment_type_mismatch,
    with_types!("PROGRAM main VAR d : DINT; s : Small; b : Big; END_VAR d := s + b; END_PROGRAM"),
    [Problem::AssignmentTypeMismatch]
);

rule_ok!(
    #[spec_test(REQ_IC_analyzer_094)]
    apply_when_argument_of_subrange_base_type_then_ok,
    with_types!(
        "PROGRAM main VAR r : DINT; s : Small; i : INT; END_VAR
         r := f(s); r := f(i); r := f(s + 1); END_PROGRAM"
    )
);

rule_err!(
    #[spec_test(REQ_IC_analyzer_094)]
    apply_when_argument_wider_than_subrange_base_type_then_arg_type_mismatch,
    with_types!("PROGRAM main VAR r : DINT; d : DINT; END_VAR r := f(d); END_PROGRAM"),
    [Problem::FunctionCallArgTypeMismatch]
);

/// The structures the field tests below declare: a field of a subrange type,
/// and a field reached through an element of an array field.
macro_rules! with_fields {
    ($program:literal) => {
        concat!(
            "TYPE R : INT (5..50);
             S : STRUCT v : R; END_STRUCT;
             Item : STRUCT a : DINT; END_STRUCT;
             Holder : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE ",
            $program
        )
    };
}

rule_ok!(
    #[spec_test(REQ_IC_analyzer_095)]
    apply_when_subrange_field_assigned_to_real_then_ok,
    with_fields!("PROGRAM main VAR s : S; r : REAL; END_VAR r := s.v; END_PROGRAM")
);

rule_err!(
    #[spec_test(REQ_IC_analyzer_095)]
    apply_when_dint_field_of_array_field_element_assigned_to_real_then_assignment_type_mismatch,
    with_fields!("PROGRAM main VAR h : Holder; r : REAL; END_VAR r := h.items[1].a; END_PROGRAM"),
    [Problem::AssignmentTypeMismatch]
);
