//! Tests for a value that is not a reference stored in or compared with a
//! reference: a reference takes and equals only `REF()`, `NULL` or another
//! reference, in every dialect.

use crate::test_helpers::{edition3_options, rule_codes};
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program declaring an `INT`, a `BOOL` and two references to `INT`,
/// whose body is `body`.
fn program(body: &str) -> String {
    format!(
        "PROGRAM Main
VAR
  i : INT;
  b : BOOL;
  ri : REF_TO INT;
  ri2 : REF_TO INT;
END_VAR
  {body}
END_PROGRAM"
    )
}

fn codes(body: &str) -> Vec<String> {
    rule_codes(super::apply, &program(body), &edition3_options())
}

#[spec_test(REQ_IC_analyzer_120)]
#[rstest]
#[case::integer_literal("ri := 0;")]
#[case::boolean_literal("ri := TRUE;")]
#[case::variable("ri := i;")]
#[case::expression("ri := i + 1;")]
#[case::parenthesized("ri := (0);")]
fn apply_when_value_not_reference_assigned_to_reference_then_p2032(#[case] body: &str) {
    assert_eq!(codes(body), [Problem::ReferenceTypeMismatch.code()]);
}

#[spec_test(REQ_IC_analyzer_120)]
#[rstest]
#[case::reference_of("ri := REF(i);")]
#[case::null("ri := NULL;")]
#[case::reference("ri2 := ri;")]
#[case::through_a_dereference("ri^ := 5;")]
fn apply_when_reference_or_dereference_assigned_then_ok(#[case] body: &str) {
    let codes = codes(body);
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_121)]
#[rstest]
#[case::integer_literal("b := ri = 0;")]
#[case::literal_on_the_left("b := 0 <> ri;")]
#[case::variable("b := ri = i;")]
#[case::value_with_null("b := i = NULL;")]
fn apply_when_reference_compared_with_value_then_p4049(#[case] body: &str) {
    assert_eq!(codes(body), [Problem::OperatorOperandTypeMismatch.code()]);
}

#[spec_test(REQ_IC_analyzer_121)]
#[rstest]
#[case::null("b := ri = NULL;")]
#[case::reference("b := ri <> ri2;")]
#[case::dereference("b := ri^ = 0;")]
fn apply_when_reference_compared_with_reference_then_ok(#[case] body: &str) {
    let codes = codes(body);
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_121)]
#[test]
fn apply_when_reference_ordered_against_value_then_p4049_only() {
    assert_eq!(
        codes("b := ri < 0;"),
        [Problem::OperatorOperandTypeMismatch.code()]
    );
}

/// A CODESYS program with a pointer to `INT`, whose body is `body`.
fn codesys_program(body: &str) -> String {
    format!(
        "PROGRAM Main
VAR
  i : INT;
  b : BOOL;
  pp : POINTER TO INT;
END_VAR
  {body}
END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_120)]
#[spec_test(REQ_IC_analyzer_121)]
#[rstest]
#[case::zero_assigned("pp := 0;", Problem::ReferenceTypeMismatch)]
#[case::compared_with_zero("b := pp <> 0;", Problem::OperatorOperandTypeMismatch)]
fn apply_when_codesys_pointer_meets_zero_then_error(#[case] body: &str, #[case] problem: Problem) {
    let options = CompilerOptions::from_dialect(Dialect::Codesys);
    assert_eq!(
        rule_codes(super::apply, &codesys_program(body), &options),
        [problem.code()]
    );
}

#[rstest]
#[case::address_of("pp := ADR(i);")]
#[case::arithmetic("pp := pp + 2;")]
fn apply_when_codesys_pointer_takes_address_or_arithmetic_then_ok(#[case] body: &str) {
    let options = CompilerOptions::from_dialect(Dialect::Codesys);
    let codes = rule_codes(super::apply, &codesys_program(body), &options);
    assert!(codes.is_empty(), "{codes:?}");
}

/// A bare store to a TwinCAT `REFERENCE TO` writes through it.
#[test]
fn apply_when_value_stored_through_reference_to_then_ok() {
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    let program = "PROGRAM Main
VAR
  i : INT;
  r : REFERENCE TO INT;
END_VAR
  r REF= i;
  r := 5;
END_PROGRAM";
    let codes = rule_codes(super::apply, program, &options);
    assert!(codes.is_empty(), "{codes:?}");
}
