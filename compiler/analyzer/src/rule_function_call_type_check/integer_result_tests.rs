//! Tests for `TRUNC` and `INT_TO_BCD` assigned to a target: `TRUNC`'s result
//! is an integer and `INT_TO_BCD`'s the bit string as wide as its input, so
//! a target of another kind, or a narrower bit string, is a mismatch rather
//! than a value the code generator silently reinterprets or truncates.

use ironplc_problems::Problem;
use spec_test_macro::spec_test;

rule_err!(
    #[spec_test(REQ_IC_analyzer_091)]
    apply_when_int_to_bcd_assigned_to_narrower_bit_string_then_return_type_mismatch,
    "PROGRAM main VAR l : LINT; w : WORD; END_VAR w := INT_TO_BCD(l); END_PROGRAM",
    [Problem::FunctionCallReturnTypeMismatch]
);

rule_err!(
    #[spec_test(REQ_IC_analyzer_091)]
    apply_when_int_to_bcd_assigned_to_integer_then_return_type_mismatch,
    "PROGRAM main VAR i : INT; j : INT; END_VAR j := INT_TO_BCD(i); END_PROGRAM",
    [Problem::FunctionCallReturnTypeMismatch]
);

rule_err!(
    #[spec_test(REQ_IC_analyzer_091)]
    apply_when_trunc_assigned_to_bit_string_then_return_type_mismatch,
    "PROGRAM main VAR r : REAL; w : WORD; END_VAR w := TRUNC(r); END_PROGRAM",
    [Problem::FunctionCallReturnTypeMismatch]
);

rule_ok!(
    apply_when_int_to_bcd_or_trunc_assigned_to_fitting_target_then_ok,
    "PROGRAM main VAR i : INT; r : REAL; w : WORD; lw : LWORD; l : LINT; x : REAL; END_VAR
     w := INT_TO_BCD(i); lw := INT_TO_BCD(i); l := TRUNC(r); x := TRUNC(r); END_PROGRAM"
);
