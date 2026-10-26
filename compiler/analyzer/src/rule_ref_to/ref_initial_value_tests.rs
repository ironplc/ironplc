//! Tests for `REF()` as a reference's initial value (`r : REF_TO INT :=
//! REF(i)`): it gets the checks `REF()` in an expression gets, the operand
//! rules and the comparison with the type the declaration references.

use crate::test_helpers::{diagnostic_codes, edition3_options, rule_codes, rule_diagnostics};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

/// A program declaring `declarations` after an `INT`, a `DINT`, an array
/// of `INT` and a structure with an `INT` field.
fn program(declarations: &str) -> String {
    format!(
        "TYPE P : STRUCT x : INT; END_STRUCT; END_TYPE
PROGRAM Main
VAR
  i : INT;
  d : DINT;
  arr : ARRAY[0..3] OF INT;
  p : P;
  {declarations}
END_VAR
END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_130)]
#[rstest]
#[case::variable_of_the_type("r : REF_TO INT := REF(i);")]
#[case::inline_array_of_the_shape("r : REF_TO ARRAY[0..3] OF INT := REF(arr);")]
#[case::null("r : REF_TO INT := NULL;")]
fn apply_when_initial_value_fits_then_ok(#[case] declarations: &str) {
    let codes = rule_codes(super::apply, &program(declarations), &edition3_options());
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_130)]
#[test]
fn apply_when_initial_value_references_another_type_then_p2032() {
    assert_eq!(
        rule_codes(
            super::apply,
            &program("r : REF_TO INT := REF(d);"),
            &edition3_options()
        ),
        [Problem::ReferenceTypeMismatch.code()]
    );
}

/// The mismatch is labelled at the operand, in the declaration, not at
/// the start of the file.
#[spec_test(REQ_IC_analyzer_130)]
#[test]
fn apply_when_initial_value_references_another_type_then_p2032_at_operand() {
    let program = program("r : REF_TO INT := REF(d);");
    let diagnostics = rule_diagnostics(super::apply, &program, &edition3_options());

    assert_eq!(
        diagnostic_codes(&diagnostics),
        [Problem::ReferenceTypeMismatch.code()]
    );
    let start = program.find("REF(d)").unwrap() + "REF(".len();
    let location = &diagnostics[0].primary.location;
    assert_eq!((location.start, location.end), (start, start + 1));
}

#[spec_test(REQ_IC_analyzer_130)]
#[test]
fn apply_when_initial_value_references_another_type_and_type_punning_allowed_then_ok() {
    let options = CompilerOptions {
        allow_ref_type_punning: true,
        ..edition3_options()
    };
    let codes = rule_codes(
        super::apply,
        &program("r : REF_TO INT := REF(d);"),
        &options,
    );
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_131)]
#[rstest]
#[case::array_element("r : REF_TO INT := REF(arr[1]);", Problem::RefOfArrayElement)]
#[case::field("r : REF_TO P := REF(p.x);", Problem::RefOperandNotVariable)]
fn apply_when_initial_value_operand_not_a_simple_variable_then_error(
    #[case] declarations: &str,
    #[case] problem: Problem,
) {
    assert_eq!(
        rule_codes(super::apply, &program(declarations), &edition3_options()),
        [problem.code()]
    );
}

/// A function block with an `INT` input `inVal` and a `VAR_TEMP` `temp`,
/// declaring `declarations`.
fn block_program(declarations: &str) -> String {
    format!(
        "FUNCTION_BLOCK FB1
VAR_INPUT
  inVal : INT;
END_VAR
VAR_TEMP
  temp : INT;
END_VAR
VAR
  {declarations}
END_VAR
END_FUNCTION_BLOCK"
    )
}

/// A function with an `INT` input `inVal`, declaring `declarations`.
fn function_program(declarations: &str) -> String {
    format!(
        "FUNCTION MyFunc : INT
VAR_INPUT
  inVal : INT;
END_VAR
VAR
  {declarations}
END_VAR
MyFunc := 0;
END_FUNCTION"
    )
}

#[spec_test(REQ_IC_analyzer_131)]
#[rstest]
#[case::var_temp(block_program("r : REF_TO INT := REF(temp);"))]
#[case::function_input(function_program("r : REF_TO INT := REF(inVal);"))]
fn apply_when_initial_value_references_ephemeral_variable_then_p2029(#[case] program: String) {
    assert_eq!(
        rule_codes(super::apply, &program, &edition3_options()),
        [Problem::RefOfEphemeralVariable.code()]
    );
}

#[spec_test(REQ_IC_analyzer_131)]
#[test]
fn apply_when_initial_value_references_block_input_then_ok() {
    let codes = rule_codes(
        super::apply,
        &block_program("r : REF_TO INT := REF(inVal);"),
        &edition3_options(),
    );
    assert!(codes.is_empty(), "{codes:?}");
}

#[spec_test(REQ_IC_analyzer_131)]
#[test]
fn apply_when_initial_value_references_ephemeral_variable_and_stack_variables_allowed_then_ok() {
    let options = CompilerOptions {
        allow_ref_stack_variables: true,
        ..edition3_options()
    };
    let codes = rule_codes(
        super::apply,
        &function_program("r : REF_TO INT := REF(inVal);"),
        &options,
    );
    assert!(codes.is_empty(), "{codes:?}");
}
