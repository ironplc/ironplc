//! Tests for an assignment through a dereference, which stores into the
//! variable the reference refers to and is checked as an assignment to that
//! variable is.

use crate::test_helpers::{edition3_options, rule_codes};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// The problem codes the rule reports for `body` inside a program that
/// declares references to a `SINT` and an `LINT`, one of them through a named
/// reference type.
fn problem_codes(body: &str) -> Vec<String> {
    let program = format!(
        "
TYPE
  SINT_REF : REF_TO SINT;
END_TYPE

FUNCTION TO_DINT : DINT
VAR_INPUT x : DINT; END_VAR
  TO_DINT := x;
END_FUNCTION

PROGRAM main
VAR
  s : SINT;
  l : LINT;
  d : DINT;
  u : UDINT;
  p : REF_TO SINT;
  r : REF_TO LINT;
  n : SINT_REF;
END_VAR
  p := REF(s);
  r := REF(l);
  n := REF(s);
  {body}
END_PROGRAM"
    );
    rule_codes(super::apply, &program, &edition3_options())
}

#[spec_test(REQ_IC_analyzer_080)]
#[rstest]
#[case::variable("p^ := d;")]
#[case::named_reference_type("n^ := d;")]
#[case::unsigned("p^ := u;")]
fn apply_when_dereference_narrows_value_then_p4035(#[case] body: &str) {
    assert_eq!(
        problem_codes(body),
        vec![Problem::AssignmentTypeMismatch.code().to_string()]
    );
}

#[spec_test(REQ_IC_analyzer_080)]
#[test]
fn apply_when_dereference_narrows_function_result_then_p4027() {
    assert_eq!(
        problem_codes("p^ := TO_DINT(d);"),
        vec![Problem::FunctionCallReturnTypeMismatch.code().to_string()]
    );
}

#[spec_test(REQ_IC_analyzer_080)]
#[rstest]
#[case::widened("r^ := d;")]
#[case::unsigned_widened("r^ := u;")]
#[case::same_type("p^ := s;")]
#[case::literal("p^ := 5; r^ := 5000000000;")]
#[case::function_result_widened("r^ := TO_DINT(d);")]
#[case::named_reference_type("n^ := s;")]
fn apply_when_dereference_of_value_assignable_to_referenced_type_then_ok(#[case] body: &str) {
    assert_eq!(problem_codes(body), Vec::<String>::new());
}
