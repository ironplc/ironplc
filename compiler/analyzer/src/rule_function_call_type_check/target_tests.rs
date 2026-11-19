//! Tests for an assignment whose target is a field or an array element,
//! which stores into a variable of the type its declaration records and is
//! checked as an assignment to a variable of that type is. See
//! https://github.com/ironplc/ironplc/issues/2284.

use crate::test_helpers::{edition3_options, rule_codes, rule_diagnostics};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// The program the tests check `body` in: a structure with an `INT`, a
/// `REAL` and a `WSTRING` field, a structure nesting it, arrays of `INT` and
/// of the structure, and a function block with a `REAL` output.
fn program(body: &str) -> String {
    format!(
        "
TYPE
  S : STRUCT f : INT; r : REAL; w : WSTRING; END_STRUCT;
  O : STRUCT inner : S; END_STRUCT;
END_TYPE

FUNCTION_BLOCK Source
VAR_OUTPUT q : REAL; END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
  s : STRING;
  i : INT;
  n : INT;
  t : S;
  o : O;
  a : ARRAY[1..2] OF INT;
  items : ARRAY[1..2] OF S;
  src : Source;
END_VAR
  {body}
END_PROGRAM"
    )
}

/// The problem codes the rule reports for `body` in [`program`].
fn problem_codes(body: &str) -> Vec<String> {
    rule_codes(super::apply, &program(body), &edition3_options())
}

#[spec_test(REQ_IC_analyzer_135)]
#[rstest]
#[case::variable("n := s;")]
#[case::field("t.f := s;")]
#[case::field_of_nested_structure("o.inner.f := s;")]
#[case::array_element("a[1] := s;")]
#[case::field_of_array_element("items[1].f := s;")]
#[case::real_field_value("t.f := t.r;")]
#[case::output_stored_in_field("src(q => t.f);")]
fn apply_when_value_does_not_fit_target_then_p4035(#[case] body: &str) {
    assert_eq!(
        problem_codes(body),
        vec![Problem::AssignmentTypeMismatch.code().to_string()]
    );
}

#[spec_test(REQ_IC_analyzer_135)]
#[rstest]
#[case::field("t.f := INT_TO_REAL(i);")]
#[case::array_element("a[1] := INT_TO_REAL(i);")]
#[case::field_of_array_element("items[2].f := INT_TO_REAL(i);")]
fn apply_when_function_result_does_not_fit_target_then_p4027(#[case] body: &str) {
    assert_eq!(
        problem_codes(body),
        vec![Problem::FunctionCallReturnTypeMismatch.code().to_string()]
    );
}

#[spec_test(REQ_IC_analyzer_135)]
#[rstest]
#[case::widened_into_field("t.r := i;")]
#[case::widened_into_field_of_nested_structure("o.inner.r := i;")]
#[case::same_type_into_element("a[1] := i;")]
#[case::literal_into_field_of_element("items[1].f := 5;")]
#[case::element_into_field("t.f := a[2];")]
#[case::function_result_into_field("t.r := INT_TO_REAL(i);")]
#[case::output_widened_into_field("src(q => o.inner.r);")]
fn apply_when_value_fits_target_then_ok(#[case] body: &str) {
    assert_eq!(problem_codes(body), Vec::<String>::new());
}

/// A string of the other encoding is P4034, which `rule_string_encoding_compat`
/// reports whatever names the target; this rule reports nothing more.
#[test]
fn apply_when_string_stored_in_wide_string_field_then_ok() {
    assert_eq!(problem_codes("t.w := s;"), Vec::<String>::new());
}

#[test]
fn apply_when_value_does_not_fit_field_then_target_context_names_field() {
    let diagnostics = rule_diagnostics(
        super::apply,
        &program("items[1].f := s;"),
        &edition3_options(),
    );

    assert_eq!(diagnostics.len(), 1, "got {diagnostics:?}");
    let target = diagnostics[0]
        .described
        .iter()
        .find(|context| context.starts_with("target="));
    assert_eq!(target.map(String::as_str), Some("target=items[1].f"));
}

#[spec_test(REQ_IC_analyzer_135)]
#[test]
fn apply_when_value_does_not_fit_member_of_this_then_p4035() {
    let program = "
FUNCTION_BLOCK Block
VAR n : INT; s : STRING; END_VAR
METHOD Run
  THIS^.n := s;
END_METHOD
END_FUNCTION_BLOCK";

    assert_eq!(
        rule_codes(super::apply, program, &edition3_options()),
        vec![Problem::AssignmentTypeMismatch.code().to_string()]
    );
}
