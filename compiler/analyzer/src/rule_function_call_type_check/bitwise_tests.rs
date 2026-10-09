//! Tests for `AND`, `OR` and `XOR` assigned to a target: the operation has
//! the type every operand widens to, so a target narrower than the widest
//! operand is a mismatch rather than a truncation.

use ironplc_problems::Problem;
use spec_test_macro::spec_test;

rule_err!(
    #[spec_test(REQ_IC_analyzer_084)]
    apply_when_bitwise_operator_of_narrow_and_wide_assigned_to_narrow_then_assignment_type_mismatch,
    "PROGRAM main VAR w : WORD; lw : LWORD; END_VAR w := w OR lw; END_PROGRAM",
    [Problem::AssignmentTypeMismatch]
);

rule_err!(
    #[spec_test(REQ_IC_analyzer_084)]
    apply_when_bitwise_function_form_of_narrow_and_wide_assigned_to_narrow_then_return_type_mismatch,
    "PROGRAM main VAR w : WORD; lw : LWORD; END_VAR w := AND(w, lw); END_PROGRAM",
    [Problem::FunctionCallReturnTypeMismatch]
);

rule_ok!(
    apply_when_bitwise_operator_of_narrow_and_wide_assigned_to_wide_then_ok,
    "PROGRAM main VAR w : WORD; lw : LWORD; r : LWORD; END_VAR r := w OR lw; r := AND(w, lw); END_PROGRAM"
);
