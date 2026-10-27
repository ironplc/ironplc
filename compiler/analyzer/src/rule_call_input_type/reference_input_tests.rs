//! Tests for a reference passed to a reference input of a function block
//! call or a method call: it is checked as an argument of a function's
//! reference parameter is, so `REF()` of another type and a reference to
//! another type are each a mismatch. A value that is not a reference is
//! left alone.

use crate::test_helpers::{diagnostic_codes, edition3_options, rule_codes, rule_diagnostics};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program with an instance `h` of a block with a `REF_TO INT` input `r`
/// and a method `Set` with a `REF_TO INT` input `m`, and an instance `n` of
/// a block with an input `k` of a named reference type and an input `a`
/// referencing an alias of `INT`, whose body is `body`.
fn program(body: &str) -> String {
    format!(
        "TYPE IntRef : REF_TO INT; MY_INT : INT; END_TYPE
FUNCTION_BLOCK Holder
VAR_INPUT r : REF_TO INT; END_VAR
METHOD Set
VAR_INPUT m : REF_TO INT; END_VAR
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Named
VAR_INPUT k : IntRef; a : REF_TO MY_INT; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
  i : INT;
  d : DINT;
  ri : REF_TO INT;
  rd : REF_TO DINT;
  h : Holder;
  n : Named;
END_VAR
  {body}
END_PROGRAM"
    )
}

fn codes(body: &str) -> Vec<String> {
    rule_codes(super::apply, &program(body), &edition3_options())
}

#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::reference_of_another_type("h(r := REF(d));")]
#[case::reference_of_a_reference("h(r := REF(ri));")]
#[case::reference_to_another_type("h(r := rd);")]
#[case::method_reference_of_another_type("h.Set(m := REF(d));")]
#[case::method_reference_to_another_type("h.Set(m := rd);")]
#[case::named_reference_type("n(k := REF(d));")]
fn apply_when_reference_input_takes_another_type_then_p4026(#[case] body: &str) {
    assert_eq!(codes(body), [Problem::FunctionCallArgTypeMismatch.code()]);
}

#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::reference_of_its_type("h(r := REF(i));")]
#[case::reference_to_its_type("h(r := ri);")]
#[case::null("h(r := NULL);")]
#[case::method_reference_of_its_type("h.Set(m := REF(i));")]
#[case::named_reference_type("n(k := REF(i));")]
#[case::reference_to_an_alias("n(a := ri);")]
fn apply_when_reference_input_takes_its_type_then_ok(#[case] body: &str) {
    let codes = codes(body);
    assert!(codes.is_empty(), "{codes:?}");
}

/// A value that is not a reference, given to a reference input, is left
/// alone: given to a TwinCAT `REFERENCE TO` input it may bind the
/// reference, which IronPLC does not implement, and the input's type does
/// not say which keyword declared it.
#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::value("h(r := d);")]
#[case::literal("h(r := 0);")]
#[case::method_value("h.Set(m := d);")]
fn apply_when_reference_input_takes_value_then_left_alone(#[case] body: &str) {
    let codes = codes(body);
    assert!(codes.is_empty(), "{codes:?}");
}

/// Type punning lets an assignment store a reference to another type
/// (P2032), not an argument: a function's reference parameter reports a
/// reference to another type with the flag on, and so does an input.
#[spec_test(REQ_IC_analyzer_132)]
#[rstest]
#[case::reference_of_another_type("h(r := REF(d));")]
#[case::reference_to_another_type("h(r := rd);")]
fn apply_when_reference_input_takes_another_type_and_type_punning_allowed_then_p4026(
    #[case] body: &str,
) {
    let options = CompilerOptions {
        allow_ref_type_punning: true,
        ..edition3_options()
    };
    assert_eq!(
        rule_codes(super::apply, &program(body), &options),
        [Problem::FunctionCallArgTypeMismatch.code()]
    );
}

/// The diagnostic names the input's type and the value's as a function's
/// reference parameter does.
#[test]
fn apply_when_reference_of_another_type_then_p4026_names_both_references() {
    let diagnostics = rule_diagnostics(
        super::apply,
        &program("h(r := REF(d));"),
        &edition3_options(),
    );

    assert_eq!(
        diagnostic_codes(&diagnostics),
        [Problem::FunctionCallArgTypeMismatch.code()]
    );
    assert_eq!(
        diagnostics[0].described,
        [
            "function_block=h",
            "parameter=r",
            "expected=REF_TO int",
            "actual=REF_TO dint"
        ]
    );
}
