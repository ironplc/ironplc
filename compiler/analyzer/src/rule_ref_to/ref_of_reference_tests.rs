//! Tests for `REF()` of a variable that is itself a reference, stored in a
//! reference by an assignment or as its initial value: a reference to a
//! reference is the nesting a declaration may not spell either (P2036).

use crate::test_helpers::{edition3_options, rule_codes};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program declaring an `INT`, a `REF_TO INT` `r` to it, an `n` of a
/// named reference type and `declarations`, whose body is `body`.
fn program(declarations: &str, body: &str) -> String {
    format!(
        "TYPE IntRef : REF_TO INT; END_TYPE
PROGRAM Main
VAR
  i : INT;
  r : REF_TO INT := REF(i);
  n : IntRef;
  {declarations}
END_VAR
  n := REF(i);
  {body}
END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::assigned("r2 : REF_TO INT;", "r2 := REF(r);")]
#[case::assigned_of_named_reference_type("r2 : REF_TO INT;", "r2 := REF(n);")]
#[case::initial_value("r2 : REF_TO INT := REF(r);", "")]
fn apply_when_reference_of_reference_stored_then_p2036(
    #[case] declarations: &str,
    #[case] body: &str,
) {
    assert_eq!(
        rule_codes(
            super::apply,
            &program(declarations, body),
            &edition3_options()
        ),
        [Problem::NestedRefToNotSupported.code()]
    );
}

#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::assigned("r2 : REF_TO INT;", "r2 := REF(r);")]
#[case::initial_value("r2 : REF_TO INT := REF(r);", "")]
fn apply_when_reference_of_reference_stored_and_type_punning_allowed_then_ok(
    #[case] declarations: &str,
    #[case] body: &str,
) {
    let options = CompilerOptions {
        allow_ref_type_punning: true,
        ..edition3_options()
    };
    let codes = rule_codes(super::apply, &program(declarations, body), &options);
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::assigned("r2 : REF_TO INT;", "r2 := REF(i);")]
#[case::initial_value("r2 : REF_TO INT := REF(i);", "")]
fn apply_when_reference_of_value_stored_then_ok(#[case] declarations: &str, #[case] body: &str) {
    let codes = rule_codes(
        super::apply,
        &program(declarations, body),
        &edition3_options(),
    );
    assert!(codes.is_empty(), "{codes:?}");
}
