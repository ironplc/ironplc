//! Tests for the conversion the pass records of a value whose type is the
//! one its function or method returns, or the one its referenced variable
//! has: a call to a user-defined function or a method, and a dereference.

use super::*;

/// A function `big` returning a `UDINT`, a function `half` returning a
/// `REAL`, and a block `K` whose method `Big` returns a `UDINT`, followed by
/// a program whose body is `body`.
fn result_program(body: &str) -> String {
    format!(
        "FUNCTION big : UDINT VAR_INPUT x : UDINT; END_VAR big := x; END_FUNCTION
        FUNCTION half : REAL VAR_INPUT x : REAL; END_VAR half := x / 2.0; END_FUNCTION
        FUNCTION_BLOCK K METHOD Big : UDINT Big := 1; END_METHOD END_FUNCTION_BLOCK
        PROGRAM main
        VAR k : K; u : UDINT; r : REAL; p : REF_TO UDINT; d : UDINT;
            l : LINT; lr : LREAL; END_VAR
        {body}
        END_PROGRAM"
    )
}

/// The values every assignment in the functions, the method and the
/// program whose body is `body` assigns, in no particular order: the
/// analyzer orders declarations by their dependencies.
fn assigned(body: &str) -> Vec<String> {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assigned_values_with(&result_program(body), &options)
}

#[spec_test(REQ_IC_analyzer_085)]
#[rstest]
#[case::function("l := big(u);", "UDINT->LINT")]
#[case::real_function("lr := half(r);", "REAL->LREAL")]
#[case::method("l := k.Big();", "UDINT->LINT")]
#[case::dereference("p := REF(u); l := p^;", "UDINT->LINT")]
fn apply_when_result_assigned_to_target_of_another_width_then_converted_to_target_type(
    #[case] body: &str,
    #[case] expected: &str,
) {
    let values = assigned(body);
    assert!(values.contains(&expected.to_string()), "{values:?}");
}

#[spec_test(REQ_IC_analyzer_085)]
#[rstest]
#[case::function("d := big(u);")]
#[case::method("d := k.Big();")]
#[case::dereference("p := REF(u); d := p^;")]
fn apply_when_result_assigned_to_target_of_its_width_then_unchanged(#[case] body: &str) {
    let values = assigned(body);
    assert!(
        values.iter().all(|value| !value.contains("->")),
        "{values:?}"
    );
}
