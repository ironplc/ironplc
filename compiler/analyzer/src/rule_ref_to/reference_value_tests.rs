//! Tests for a reference variable assigned to a variable: it fits only a
//! reference to the same type, since a reference holds the index of the
//! variable it refers to, not that variable's value.

use crate::test_helpers::{edition3_options, rule_codes};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program declaring an `INT`, a `DINT`, references to each and an
/// instance of a block with a reference input, whose body is `body`.
fn program(body: &str) -> String {
    format!(
        "FUNCTION_BLOCK Holder
VAR_INPUT r : REF_TO INT; END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
  i : INT;
  d : DINT;
  ri : REF_TO INT;
  ri2 : REF_TO INT;
  rd : REF_TO DINT;
  h : Holder;
END_VAR
  ri := REF(i);
  rd := REF(d);
  {body}
END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_087)]
#[rstest]
#[case::to_a_value("i := ri;")]
#[case::to_a_reference_to_another_type("ri := rd;")]
fn apply_when_reference_assigned_where_it_does_not_fit_then_p2032(#[case] body: &str) {
    assert_eq!(
        rule_codes(super::apply, &program(body), &edition3_options()),
        [Problem::ReferenceTypeMismatch.code()]
    );
}

#[spec_test(REQ_IC_analyzer_087)]
#[rstest]
#[case::to_a_reference_to_its_type("ri2 := ri;")]
#[case::through_a_dereference("i := ri^;")]
#[case::to_a_field_not_looked_up("h.r := ri;")]
fn apply_when_reference_assigned_where_it_fits_then_ok(#[case] body: &str) {
    let codes = rule_codes(super::apply, &program(body), &edition3_options());
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_087)]
#[test]
fn apply_when_reference_to_another_type_and_type_punning_allowed_then_ok() {
    let options = CompilerOptions {
        allow_ref_type_punning: true,
        ..edition3_options()
    };
    let codes = rule_codes(super::apply, &program("ri := rd;"), &options);
    assert!(codes.is_empty(), "{codes:?}");
}
