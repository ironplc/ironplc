//! Tests for an input of a function block call or an argument of a method
//! call whose variable is a reference: it takes a reference to the type the
//! variable references, written as a reference variable or as `REF()`, or
//! `NULL`, as a function's `REF_TO` parameter does.

use crate::test_helpers::{edition3_options, rule_codes};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A block `Holder` with a `REF_TO INT` input `rr`, an input `ra`
/// referencing an alias of `INT`, an input `pa` referencing an inline
/// array and a method `Go` with a `REF_TO INT` input `mr`, followed by a
/// program declaring an `INT`, a `DINT`, references to each, two arrays
/// and an instance `h`, whose body is `body`.
fn program(body: &str) -> String {
    format!(
        "TYPE MY_INT : INT; END_TYPE
FUNCTION_BLOCK Holder
VAR_INPUT
  rr : REF_TO INT;
  ra : REF_TO MY_INT;
  pa : REF_TO ARRAY[1..3] OF DINT;
END_VAR
METHOD Go
VAR_INPUT mr : REF_TO INT; END_VAR
END_METHOD
END_FUNCTION_BLOCK
PROGRAM main
VAR
  i : INT;
  d : DINT;
  ri : REF_TO INT;
  rd : REF_TO DINT;
  arr : ARRAY[1..3] OF DINT;
  wide : ARRAY[1..4] OF DINT;
  h : Holder;
END_VAR
  ri := REF(i);
  rd := REF(d);
  {body}
END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_129)]
#[rstest]
#[case::reference_of_another_type("h(rr := REF(d));")]
#[case::reference_to_another_type("h(rr := rd);")]
#[case::value_not_a_reference("h(rr := i);")]
#[case::reference_of_array_of_another_shape("h(pa := REF(wide));")]
#[case::method_reference_of_another_type("h.Go(mr := REF(d));")]
#[case::method_reference_to_another_type("h.Go(mr := rd);")]
fn apply_when_reference_input_not_accepted_then_p4026(#[case] body: &str) {
    assert_eq!(
        rule_codes(super::apply, &program(body), &edition3_options()),
        [Problem::FunctionCallArgTypeMismatch.code()]
    );
}

#[spec_test(REQ_IC_analyzer_129)]
#[rstest]
#[case::reference_of_the_type("h(rr := REF(i));")]
#[case::reference_to_the_type("h(rr := ri);")]
#[case::null("h(rr := NULL);")]
#[case::reference_of_alias_of_the_type("h(ra := REF(i));")]
#[case::reference_of_array_of_the_shape("h(pa := REF(arr));")]
#[case::method_reference_of_the_type("h.Go(mr := REF(i));")]
#[case::method_reference_to_the_type("h.Go(mr := ri);")]
fn apply_when_reference_input_accepted_then_ok(#[case] body: &str) {
    let codes = rule_codes(super::apply, &program(body), &edition3_options());
    assert!(codes.is_empty(), "{codes:?}");
}
