//! Tests for an output assignment of a function block call, which stores the
//! output read from the instance after the call and is checked as the
//! assignment of that read is: `f(q => v)` as `v := f.q`.

use crate::test_helpers::{edition3_options, rule_diagnostics};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program whose body is `body`, and which declares a block whose outputs
/// are an `LINT`, a `BOOL`, a `REAL`, a `SINT` and a `STRING`.
fn program(body: &str) -> String {
    format!(
        "
FUNCTION_BLOCK FB
VAR_OUTPUT ql : LINT; qb : BOOL; qr : REAL; qs : SINT; qstr : STRING; END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
  f : FB;
  t : TON;
  i : INT;
  d : DINT;
  l : LINT;
  lr : LREAL;
  b : BOOL;
  tm : TIME;
  lt : LTIME;
  w : WSTRING;
END_VAR
  {body}
END_PROGRAM"
    )
}

/// The diagnostics the rule reports for `body`.
fn diagnostics(body: &str) -> Vec<ironplc_dsl::diagnostic::Diagnostic> {
    rule_diagnostics(super::apply, &program(body), &edition3_options())
}

fn problem_codes(body: &str) -> Vec<String> {
    diagnostics(body)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[spec_test(REQ_IC_analyzer_106)]
#[rstest]
#[case::narrower("f(ql => i);")]
#[case::bool_to_integer("f(qb => d);")]
#[case::real_to_integer("f(qr => d);")]
#[case::negated_bool_to_integer("f(NOT qb => d);")]
#[case::standard_block("t(ET => d);")]
#[case::string_to_wstring("f(qstr => w);")]
fn apply_when_output_not_assignable_to_target_then_p4035(#[case] body: &str) {
    assert_eq!(
        problem_codes(body),
        vec![Problem::AssignmentTypeMismatch.code().to_string()]
    );
}

#[spec_test(REQ_IC_analyzer_106)]
#[rstest]
#[case::narrower("f(ql => i);", "f(); i := f.ql;")]
#[case::string_to_wstring("f(qstr => w);", "f(); w := f.qstr;")]
fn apply_when_output_not_assignable_then_same_as_assignment_of_output(
    #[case] output: &str,
    #[case] assignment: &str,
) {
    assert_eq!(problem_codes(assignment), problem_codes(output));
}

#[spec_test(REQ_IC_analyzer_106)]
#[rstest]
#[case::same_type("f(ql => l);")]
#[case::wider_integer("f(qs => i);")]
#[case::wider_real("f(qr => lr);")]
#[case::negated_bool("f(NOT qb => b);")]
#[case::standard_block("t(ET => tm);")]
#[case::standard_block_wider("t(ET => lt);")]
fn apply_when_output_assignable_to_target_then_ok(#[case] body: &str) {
    assert!(problem_codes(body).is_empty());
}

#[test]
fn apply_when_output_not_assignable_then_labels_output_where_named() {
    let body = "f(ql => i);";
    let found = diagnostics(body);
    assert_eq!(1, found.len(), "expected one diagnostic, got {found:?}");
    let label = &found[0].primary;
    assert_eq!("Output value", label.message);
    assert_eq!(
        "ql",
        &program(body)[label.location.start..label.location.end]
    );
}
