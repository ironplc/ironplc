//! Tests for a reference initialized with `REF()` in its declaration:
//! `x : REF_TO INT := REF(d)` stores the reference as `x := REF(d)` does,
//! so it reports what that assignment reports, under the same flags.

use crate::test_helpers::{
    codes, diagnostic_codes, edition3_options, rule_codes, rule_diagnostics,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program declaring an `INT`, a `DINT`, an array, a structure, a
/// reference to `INT` and two arrays spelled in place, followed by
/// `declarations`, whose body is `body`.
fn program(declarations: &str, body: &str) -> String {
    format!(
        "TYPE Point : STRUCT a : INT; END_STRUCT; END_TYPE
PROGRAM Main
VAR
  i : INT;
  d : DINT;
  arr : ARRAY[0..3] OF INT;
  s : Point;
  r : REF_TO INT;
  a3 : ARRAY[1..3] OF DINT;
  a4 : ARRAY[1..4] OF DINT;
  {declarations}
END_VAR
  {body}
END_PROGRAM"
    )
}

/// The codes the rule reports for `x : {target} := REF({operand})` and for
/// `x := REF({operand})` on `x : {target}`, under `options`.
fn initialized_and_assigned(
    target: &str,
    operand: &str,
    options: &CompilerOptions,
) -> (Vec<String>, Vec<String>) {
    let initialized = program(&format!("x : {target} := REF({operand});"), "");
    let assigned = program(&format!("x : {target};"), &format!("x := REF({operand});"));
    (
        rule_codes(super::apply, &initialized, options),
        rule_codes(super::apply, &assigned, options),
    )
}

#[spec_test(REQ_IC_analyzer_133)]
#[rstest]
#[case::same_type("REF_TO INT", "i", &[])]
#[case::reference_to_a_reference("REF_TO REF_TO INT", "r", &[])]
#[case::array_spelled_in_place("REF_TO ARRAY[1..3] OF DINT", "a3", &[])]
#[case::other_array_spelled_in_place("REF_TO ARRAY[1..3] OF DINT", "a4", &[])]
#[case::another_type("REF_TO INT", "d", &[Problem::ReferenceTypeMismatch])]
#[case::a_reference_for_a_value("REF_TO INT", "r", &[Problem::ReferenceTypeMismatch])]
#[case::a_value_for_a_reference("REF_TO REF_TO INT", "i", &[Problem::ReferenceTypeMismatch])]
#[case::array_element("REF_TO INT", "arr[1]", &[Problem::RefOfArrayElement])]
#[case::structure_field("REF_TO INT", "s.a", &[Problem::RefOperandNotVariable])]
fn apply_when_initialized_with_ref_then_as_assigned(
    #[case] target: &str,
    #[case] operand: &str,
    #[case] expected: &[Problem],
) {
    let (initialized, assigned) = initialized_and_assigned(target, operand, &edition3_options());

    assert_eq!(initialized, codes(expected));
    assert_eq!(assigned, codes(expected));
}

#[spec_test(REQ_IC_analyzer_133)]
#[rstest]
#[case::another_type("REF_TO INT", "d")]
#[case::a_reference_for_a_value("REF_TO INT", "r")]
fn apply_when_initialized_with_ref_of_another_type_and_type_punning_allowed_then_ok(
    #[case] target: &str,
    #[case] operand: &str,
) {
    let options = CompilerOptions {
        allow_ref_type_punning: true,
        ..edition3_options()
    };
    let (initialized, assigned) = initialized_and_assigned(target, operand, &options);

    assert_eq!(initialized, codes(&[]));
    assert_eq!(assigned, codes(&[]));
}

#[spec_test(REQ_IC_analyzer_133)]
#[test]
fn apply_when_initialized_with_null_then_ok() {
    let codes = rule_codes(
        super::apply,
        &program("x : REF_TO INT := NULL;", ""),
        &edition3_options(),
    );
    assert!(codes.is_empty(), "{codes:?}");
}

/// A block whose `VAR_TEMP` `t` and a function whose input `p` are
/// stack-allocated, each with a reference initialized with `REF()` of it.
const STACK_VARIABLES: &str = "FUNCTION_BLOCK Block
VAR_TEMP t : INT; END_VAR
VAR rt : REF_TO INT := REF(t); END_VAR
END_FUNCTION_BLOCK
FUNCTION F : INT
VAR_INPUT p : INT; END_VAR
VAR rp : REF_TO INT := REF(p); END_VAR
F := 0;
END_FUNCTION";

#[spec_test(REQ_IC_analyzer_133)]
#[test]
fn apply_when_initialized_with_ref_of_stack_variable_then_p2029() {
    assert_eq!(
        rule_codes(super::apply, STACK_VARIABLES, &edition3_options()),
        [Problem::RefOfEphemeralVariable.code(); 2]
    );
}

#[spec_test(REQ_IC_analyzer_133)]
#[test]
fn apply_when_initialized_with_ref_of_stack_variable_and_stack_variables_allowed_then_ok() {
    let options = CompilerOptions {
        allow_ref_stack_variables: true,
        ..edition3_options()
    };
    let codes = rule_codes(super::apply, STACK_VARIABLES, &options);
    assert!(codes.is_empty(), "{codes:?}");
}

/// The mismatch is labelled at the operand, as it is for the assignment.
#[test]
fn apply_when_initialized_with_ref_of_another_type_then_p2032_at_operand() {
    let program = program("x : REF_TO INT := REF(d);", "");
    let diagnostics = rule_diagnostics(super::apply, &program, &edition3_options());

    assert_eq!(
        diagnostic_codes(&diagnostics),
        [Problem::ReferenceTypeMismatch.code()]
    );
    let start = program.find("REF(d)").unwrap() + "REF(".len();
    let location = &diagnostics[0].primary.location;
    assert_eq!((location.start, location.end), (start, start + 1));
}
