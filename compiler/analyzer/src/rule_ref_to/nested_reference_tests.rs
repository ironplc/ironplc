//! Tests for references to references: a `REF_TO REF_TO INT` is declared,
//! made with `REF()` of a reference, followed one level at a time, and
//! checked at each level as a reference to a value is. A dereference of a
//! value that is not a reference (P2031) is one level too many, and only a
//! TwinCAT `REFERENCE TO` does not nest (P2036).

use crate::test_helpers::{diagnostic_codes, edition3_options, rule_codes, rule_diagnostics};
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program declaring an `INT`, a `DINT`, a `BOOL`, references to the
/// `INT` and the `DINT`, references to those references, an array of
/// references and an instance of a block with a reference input, whose
/// body is `body`.
fn program(body: &str) -> String {
    format!(
        "FUNCTION F : INT
F := 5;
END_FUNCTION
FUNCTION_BLOCK Holder
VAR_INPUT rr : REF_TO INT; END_VAR
END_FUNCTION_BLOCK
TYPE IntRef : REF_TO INT; END_TYPE
PROGRAM Main
VAR
  i : INT;
  d : DINT;
  b : BOOL;
  r : REF_TO INT;
  r2 : REF_TO INT;
  rd : REF_TO DINT;
  rr : REF_TO REF_TO INT;
  rr2 : REF_TO REF_TO INT;
  rn : REF_TO IntRef;
  ra : ARRAY[0..1] OF REF_TO INT;
  h : Holder;
END_VAR
  r := REF(i);
  {body}
END_PROGRAM"
    )
}

fn codes(body: &str) -> Vec<String> {
    rule_codes(super::apply, &program(body), &edition3_options())
}

#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::dereference_of_a_value("i := r^^;")]
#[case::parenthesized_dereference("i := (r^)^;")]
#[case::literal("i := 5^;")]
#[case::function_result("i := F()^;")]
#[case::sum("i := (i + 1)^;")]
#[case::three_levels_of_two("i := rr^^^;")]
fn apply_when_value_not_reference_dereferenced_then_p2031(#[case] body: &str) {
    assert_eq!(codes(body), [Problem::DerefRequiresReferenceType.code()]);
}

#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::variable("i := r^;")]
#[case::parenthesized_variable("i := (r)^;")]
#[case::reference_of("i := REF(i)^;")]
#[case::array_element("i := ra[0]^;")]
#[case::field("h(rr := REF(i)); i := h.rr^;")]
#[case::reference_to_reference("i := rr^^;")]
#[case::parenthesized_reference_to_reference("i := (rr^)^;")]
#[case::named_reference_to_reference("i := rn^^;")]
fn apply_when_reference_dereferenced_then_ok(#[case] body: &str) {
    let codes = codes(body);
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_133)]
#[rstest]
#[case::reference_of_a_reference("rr := REF(r);")]
#[case::named_reference_of_a_reference("rn := REF(r);")]
#[case::reference_to_reference_copied("rr2 := rr;")]
#[case::reference_stored_through("rr^ := REF(i); rr^ := r;")]
#[case::value_stored_through_both("rr^^ := 5;")]
#[case::value_read_through_both("i := rr^^;")]
#[case::reference_read_through("r2 := rr^;")]
#[case::null_stored_through("rr^ := NULL;")]
#[case::compared("b := rr = NULL; b := rr^ = NULL; b := rr^ = r; b := rr^^ = 0;")]
fn apply_when_reference_to_reference_used_at_its_level_then_ok(#[case] body: &str) {
    let codes = codes(body);
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_133)]
#[rstest]
#[case::reference_of_a_reference_to_a_reference_to_a_value("r := REF(r);")]
#[case::reference_of_a_value_to_a_reference_to_a_reference("rr := REF(i);")]
#[case::reference_to_a_reference_to_a_reference("rr := r;")]
#[case::reference_read_through_to_a_value("i := rr^;")]
#[case::value_stored_through_to_a_reference("rr^ := 5;")]
#[case::reference_to_another_type_stored_through("rr^ := rd;")]
#[case::reference_to_reference_to_a_value("r := rr;")]
fn apply_when_reference_to_reference_used_at_another_level_then_p2032(#[case] body: &str) {
    assert_eq!(codes(body), [Problem::ReferenceTypeMismatch.code()]);
}

#[spec_test(REQ_IC_analyzer_133)]
#[rstest]
#[case::through_both("rr^^ := NULL;")]
#[case::through_one("r^ := NULL;")]
fn apply_when_null_stored_through_to_a_value_then_p2034(#[case] body: &str) {
    assert_eq!(codes(body), [Problem::NullRequiresReferenceType.code()]);
}

#[spec_test(REQ_IC_analyzer_133)]
#[rstest]
#[case::reference_to_a_reference("rr := r;")]
#[case::reference_of_a_reference("r := REF(r);")]
fn apply_when_level_differs_and_type_punning_allowed_then_ok(#[case] body: &str) {
    let options = CompilerOptions {
        allow_ref_type_punning: true,
        ..edition3_options()
    };
    let codes = rule_codes(super::apply, &program(body), &options);
    assert!(codes.is_empty(), "{codes:?}");
}

/// A CODESYS program with a pointer to `INT` and a pointer to that pointer,
/// whose body is `body`.
fn codesys_program(body: &str) -> String {
    format!(
        "PROGRAM Main
VAR
  i : INT;
  p : POINTER TO INT;
  pp : POINTER TO POINTER TO INT;
END_VAR
  p := ADR(i);
  {body}
END_PROGRAM"
    )
}

#[spec_test(REQ_PTR_analyzer_417)]
#[rstest]
#[case::address_of_a_pointer("pp := ADR(p);")]
#[case::through_both("pp := ADR(p); pp^^ := 3; i := pp^^;")]
fn apply_when_pointer_to_pointer_then_ok(#[case] body: &str) {
    let options = CompilerOptions::from_dialect(Dialect::Codesys);
    let codes = rule_codes(super::apply, &codesys_program(body), &options);
    assert!(codes.is_empty(), "{codes:?}");
}

/// TwinCAT options with `REF_TO` enabled as well, so a program can declare
/// a `REF_TO` type and a `REFERENCE TO` it.
fn twincat_with_ref_to() -> CompilerOptions {
    CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::from_dialect(Dialect::TwinCat)
    }
}

#[spec_test(REQ_IC_analyzer_134)]
#[rstest]
#[case::reference_to_a_named_reference(
    "TYPE IntRef : REF_TO INT; END_TYPE
PROGRAM Main VAR rr : REFERENCE TO IntRef; END_VAR END_PROGRAM"
)]
#[case::reference_to_a_reference_spelled_in_place(
    "PROGRAM Main VAR rr : REFERENCE TO REF_TO INT; END_VAR END_PROGRAM"
)]
#[case::pointer_to_a_reference_spelled_in_place(
    "PROGRAM Main VAR pp : POINTER TO REFERENCE TO INT; END_VAR END_PROGRAM"
)]
#[case::type_referencing_a_reference(
    "TYPE IntRef : REF_TO INT; RR : REFERENCE TO IntRef; END_TYPE
PROGRAM Main END_PROGRAM"
)]
#[case::reference_to_a_pointer(
    "PROGRAM Main VAR rp : REFERENCE TO POINTER TO INT; END_VAR END_PROGRAM"
)]
fn apply_when_reference_to_nests_then_p2036(#[case] program: &str) {
    assert_eq!(
        rule_codes(super::apply, program, &twincat_with_ref_to()),
        [Problem::NestedRefToNotSupported.code()]
    );
}

#[spec_test(REQ_IC_analyzer_134)]
#[rstest]
#[case::reference_to_a_value("PROGRAM Main VAR r : REFERENCE TO INT; END_VAR END_PROGRAM")]
#[case::pointer_to_a_pointer(
    "PROGRAM Main VAR pp : POINTER TO POINTER TO INT; END_VAR END_PROGRAM"
)]
#[case::pointer_to_a_named_reference(
    "TYPE IntRef : REF_TO INT; END_TYPE
PROGRAM Main VAR pr : POINTER TO IntRef; END_VAR END_PROGRAM"
)]
fn apply_when_reference_to_does_not_nest_then_ok(#[case] program: &str) {
    let codes = rule_codes(super::apply, program, &twincat_with_ref_to());
    assert!(codes.is_empty(), "{codes:?}");
}

/// A nested `REFERENCE TO` spelled in place is labelled at the variable,
/// since the elementary type it ends in has no span of its own.
#[spec_test(REQ_IC_analyzer_134)]
#[test]
fn apply_when_reference_to_nests_in_place_then_p2036_at_variable() {
    let program = "PROGRAM Main VAR pp : POINTER TO REFERENCE TO INT; END_VAR END_PROGRAM";
    let diagnostics = rule_diagnostics(super::apply, program, &twincat_with_ref_to());
    assert_eq!(
        diagnostic_codes(&diagnostics),
        [Problem::NestedRefToNotSupported.code()]
    );
    let start = program.find("pp :").unwrap();
    let location = &diagnostics[0].primary.location;
    assert_eq!((location.start, location.end), (start, start + "pp".len()));
}
