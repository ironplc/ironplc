//! The type of a call to a function of several inputs of one type: the type
//! every input of that type widens to, or `EXPT`'s first input's.

use super::*;
use spec_test_macro::spec_test;

/// The type of `value` assigned in a program declaring one variable of each
/// type the tests use.
fn call_type(value: &str) -> Option<String> {
    let program = format!(
        "PROGRAM main
         VAR x : LINT; s : SINT; i : INT; d : DINT; u : UDINT; l : LINT; r : REAL; lr : LREAL;
             g : BOOL; k : INT; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let resolved = run_pass(&program);
    type_name_upper(&collect_assignment_types(&resolved)[0])
}

#[spec_test(REQ_IC_analyzer_070)]
#[rstest]
#[case::widest_last("MAX(i, l)", "LINT")]
#[case::widest_first("MAX(l, i)", "LINT")]
#[case::min("MIN(i, l)", "LINT")]
#[case::limit("LIMIT(i, d, l)", "LINT")]
#[case::sel_after_its_selector("SEL(g, i, l)", "LINT")]
#[case::mux_after_its_selector("MUX(k, s, d, l)", "LINT")]
#[case::atan2("ATAN2(r, lr)", "LREAL")]
#[case::same_type("MAX(d, d)", "DINT")]
fn apply_when_inputs_of_one_type_then_type_every_input_widens_to(
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(call_type(value), Some(expected.to_string()));
}

#[spec_test(REQ_IC_analyzer_071)]
#[rstest]
#[case::signed_first("MAX(d, u)", "DINT")]
#[case::unsigned_first("MAX(u, d)", "UDINT")]
#[case::literal_first("MAX(5, d)", "DINT")]
fn apply_when_no_input_type_accepts_every_other_then_first_concrete_input_type(
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(call_type(value), Some(expected.to_string()));
}

#[test]
fn apply_when_inputs_all_untyped_literals_then_literal_category() {
    assert_eq!(call_type("MAX(1, 2)"), Some("ANY_INT".to_string()));
}

#[spec_test(REQ_IC_analyzer_072)]
#[rstest]
#[case::wider_exponent("EXPT(r, l)", "REAL")]
#[case::narrower_exponent("EXPT(l, i)", "LINT")]
fn apply_when_expt_then_type_of_its_first_input(#[case] value: &str, #[case] expected: &str) {
    assert_eq!(call_type(value), Some(expected.to_string()));
}
