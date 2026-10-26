//! Tests for an argument passed to a `REF_TO` parameter: the argument is a
//! reference, known by `REF_TO` and the type it references, so a reference
//! to another type is a mismatch and a reference to the parameter's type,
//! under whatever name, is not.

use crate::test_helpers::{edition3_options, rule_codes};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A function `f` with a `REF_TO INT` parameter and a function `g` with a
/// parameter referencing `MY_INT`, an alias of `INT`, followed by a program
/// whose body is `body`.
fn program(body: &str) -> String {
    format!(
        "TYPE MY_INT : INT; END_TYPE
FUNCTION f : INT
VAR_INPUT r : REF_TO INT; END_VAR
  f := r^;
END_FUNCTION
FUNCTION g : INT
VAR_INPUT r : REF_TO MY_INT; END_VAR
  g := r^;
END_FUNCTION
PROGRAM main
VAR i : INT; d : DINT; ri : REF_TO INT; rd : REF_TO DINT; x : INT; END_VAR
  ri := REF(i);
  rd := REF(d);
  {body}
END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_086)]
#[test]
fn apply_when_reference_to_another_type_passed_to_ref_to_parameter_then_p4026() {
    assert_eq!(
        rule_codes(super::apply, &program("x := f(rd);"), &edition3_options()),
        [Problem::FunctionCallArgTypeMismatch.code()]
    );
}

#[spec_test(REQ_IC_analyzer_086)]
#[rstest]
#[case::variable("x := f(ri);")]
#[case::ref_of_variable("x := f(REF(i));")]
#[case::parameter_of_alias("x := g(ri);")]
fn apply_when_reference_to_parameter_type_passed_then_ok(#[case] body: &str) {
    let codes = rule_codes(super::apply, &program(body), &edition3_options());
    assert!(codes.is_empty(), "{codes:?}");
}

/// A function `f` whose parameter references an array spelled out in place,
/// followed by a program whose body is `body`.
fn array_program(body: &str) -> String {
    format!(
        "TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
FUNCTION f : INT
VAR_INPUT a : REF_TO ARRAY[1..3] OF DINT; END_VAR
  f := 0;
END_FUNCTION
PROGRAM main
VAR arr : ARRAY[1..3] OF DINT; row : Row; wide : ARRAY[1..4] OF DINT; d : DINT; x : INT; END_VAR
  {body}
END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_086)]
#[rstest]
#[case::inline_array("x := f(REF(arr));")]
#[case::named_array("x := f(REF(row));")]
fn apply_when_reference_to_array_of_parameter_shape_passed_then_ok(#[case] body: &str) {
    let codes = rule_codes(super::apply, &array_program(body), &edition3_options());
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_086)]
#[rstest]
#[case::element_type("x := f(REF(d));")]
#[case::other_shape("x := f(REF(wide));")]
fn apply_when_reference_to_other_than_parameter_array_passed_then_p4026(#[case] body: &str) {
    assert_eq!(
        rule_codes(super::apply, &array_program(body), &edition3_options()),
        [Problem::FunctionCallArgTypeMismatch.code()]
    );
}
