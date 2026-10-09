//! The type of `AND`, `OR` and `XOR`, in the operator and the function form:
//! the type every operand widens to.

use super::*;
use spec_test_macro::spec_test;

/// The type of `value` assigned in a program declaring one variable of each
/// type the tests use.
fn bitwise_type(value: &str) -> Option<String> {
    let program = format!(
        "PROGRAM main
         VAR x : LWORD; b : BYTE; w : WORD; d : DWORD; lw : LWORD; g : BOOL; h : BOOL; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let resolved = run_pass(&program);
    type_name_upper(&collect_assignment_types(&resolved)[0])
}

#[spec_test(REQ_IC_analyzer_081)]
#[rstest]
#[case::widest_right("w OR lw", "LWORD")]
#[case::widest_left("lw OR w", "LWORD")]
#[case::and("b AND w", "WORD")]
#[case::xor("d XOR lw", "LWORD")]
#[case::same_width("w AND d", "DWORD")]
#[case::function_form("AND(w, lw)", "LWORD")]
#[case::extensible_function_form("OR(b, lw, w)", "LWORD")]
#[case::booleans("g AND h", "BOOL")]
fn apply_when_bitwise_operation_then_type_every_operand_widens_to(
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(bitwise_type(value), Some(expected.to_string()));
}
