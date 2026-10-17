//! Tests for an output assignment of a call, which stores the output the
//! call leaves and is checked as the assignment of that output is:
//! `f(q => v)` on a function block as `v := f.q`.

use crate::test_helpers::{edition3_options, rule_diagnostics};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program whose body is `body`, and which declares a block whose outputs
/// are an `LINT`, a `BOOL`, a `REAL`, a `SINT` and a `STRING`, a method `M`
/// of it and a function `F`, each with an `LINT` and a `BOOL` output.
fn program(body: &str) -> String {
    format!(
        "
FUNCTION_BLOCK FB
VAR_OUTPUT ql : LINT; qb : BOOL; qr : REAL; qs : SINT; qstr : STRING; END_VAR
METHOD M
VAR_OUTPUT mq : LINT; mb : BOOL; END_VAR
END_METHOD
END_FUNCTION_BLOCK

FUNCTION F : DINT
VAR_INPUT x : DINT; END_VAR
VAR_OUTPUT fq : LINT; fb : BOOL; END_VAR
F := x;
END_FUNCTION

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
  r : DINT;
END_VAR
  {body}
END_PROGRAM"
    )
}

/// The diagnostics the rule reports for `body`.
fn diagnostics(body: &str) -> Vec<ironplc_dsl::diagnostic::Diagnostic> {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..edition3_options()
    };
    rule_diagnostics(super::apply, &program(body), &options)
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
#[case::function("r := F(x := 1, fq => i);")]
#[case::method("f.M(mq => i);")]
fn apply_when_output_not_assignable_to_target_then_p4035(#[case] body: &str) {
    assert_eq!(
        problem_codes(body),
        vec![Problem::AssignmentTypeMismatch.code().to_string()]
    );
}

/// A `STRING` stored in a `WSTRING` is P4034, which
/// `rule_string_encoding_compat` reports for a field read and an output
/// alike, so this rule reports nothing for either.
#[spec_test(REQ_IC_analyzer_108)]
#[rstest]
#[case::field_read("f(); w := f.qstr;")]
#[case::output("f(qstr => w);")]
fn apply_when_string_output_stored_in_wstring_then_left_to_encoding_rule(#[case] body: &str) {
    assert!(problem_codes(body).is_empty());
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
#[case::function("r := F(x := 1, fq => l);")]
#[case::method("f.M(mq => l);")]
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

#[spec_test(REQ_CB_analyzer_026)]
#[rstest]
#[case::function_block("f(NOT ql => l);")]
#[case::standard_block("t(IN := TRUE, NOT ET => tm);")]
#[case::function("r := F(x := 1, NOT fq => l);")]
#[case::method("f.M(NOT mq => l);")]
#[case::target_of_other_type("f(NOT ql => b);")]
fn apply_when_negated_output_not_bool_then_only_p4083(#[case] body: &str) {
    assert_eq!(
        problem_codes(body),
        vec![Problem::NegatedOutputNotBool.code().to_string()]
    );
}

#[spec_test(REQ_CB_analyzer_026)]
#[rstest]
#[case::function_block("f(NOT qb => b);")]
#[case::function("r := F(x := 1, NOT fb => b);")]
#[case::method("f.M(NOT mb => b);")]
fn apply_when_negated_output_bool_then_ok(#[case] body: &str) {
    assert!(problem_codes(body).is_empty());
}
