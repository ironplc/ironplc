//! Tests for a field that is a reference: a `REF_TO` field of a function
//! block instance or a structure has the type its declaration records, so
//! the rule judges it as it judges a reference variable wherever it asks
//! whether a variable is a reference.

use crate::test_helpers::{edition3_options, rule_codes};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program declaring an instance `h` of a block with a `REF_TO INT`
/// input `rr` and an `INT` input `i`, an instance `e` of a block that
/// inherits them, an array `hs` of the block, a structure `s` with a field
/// `rp` of the named reference type `IntRef`, an `INT`, a `REAL` and
/// references to `INT` and `DINT`, whose body is `body`.
fn program(body: &str) -> String {
    format!(
        "TYPE IntRef : REF_TO INT; Pair : STRUCT rp : IntRef; n : INT; END_STRUCT; END_TYPE
FUNCTION_BLOCK H
VAR_INPUT rr : REF_TO INT; i : INT; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK Extended EXTENDS H
END_FUNCTION_BLOCK
PROGRAM Main
VAR
  x : INT := 5;
  r : REAL;
  y : INT;
  b : BOOL;
  h : H;
  e : Extended;
  hs : ARRAY[1..2] OF H;
  s : Pair;
  ri : REF_TO INT;
  rd : REF_TO DINT;
END_VAR
  {body}
END_PROGRAM"
    )
}

fn codes(body: &str) -> Vec<String> {
    rule_codes(super::apply, &program(body), &edition3_options())
}

#[spec_test(REQ_IC_analyzer_131)]
#[rstest]
#[case::inherited_field_dereferenced("y := e.rr^;")]
#[case::field_of_array_element_dereferenced("y := hs[1].rr^;")]
#[case::structure_field_dereferenced("s.rp := REF(x); y := s.rp^;")]
#[case::null_assigned("h.rr := NULL;")]
#[case::reference_of_its_type_assigned("h.rr := REF(x);")]
#[case::reference_to_its_type_assigned("h.rr := ri;")]
#[case::compared_with_null("b := h.rr = NULL;")]
fn apply_when_field_is_reference_then_ok(#[case] body: &str) {
    let codes = codes(body);
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_131)]
#[rstest]
#[case::value_assigned("h.rr := 0;", Problem::ReferenceTypeMismatch)]
#[case::reference_of_another_type_assigned("h.rr := REF(r);", Problem::ReferenceTypeMismatch)]
#[case::reference_to_another_type_assigned("h.rr := rd;", Problem::ReferenceTypeMismatch)]
#[case::added_to("y := h.rr + 1;", Problem::ArithmeticOnReference)]
#[case::compared_with_value("b := h.rr = 0;", Problem::OperatorOperandTypeMismatch)]
fn apply_when_reference_field_misused_then_error(#[case] body: &str, #[case] problem: Problem) {
    assert_eq!(codes(body), [problem.code()]);
}

#[spec_test(REQ_IC_analyzer_131)]
#[rstest]
#[case::dereferenced("y := h.i^;", Problem::DerefRequiresReferenceType)]
#[case::null_assigned("h.i := NULL;", Problem::NullRequiresReferenceType)]
#[case::reference_of_assigned("h.i := REF(x);", Problem::ReferenceTypeMismatch)]
#[case::reference_assigned("h.i := ri;", Problem::ReferenceTypeMismatch)]
fn apply_when_field_is_not_reference_then_error(#[case] body: &str, #[case] problem: Problem) {
    assert_eq!(codes(body), [problem.code()]);
}

rule_ok!(
    #[spec_test(REQ_IC_analyzer_131)]
    apply_when_member_of_this_is_reference_then_ok,
    "FUNCTION_BLOCK H
VAR_INPUT rr : REF_TO INT; END_VAR
METHOD Get : INT
Get := THIS^.rr^;
END_METHOD
END_FUNCTION_BLOCK",
    edition3_options()
);
