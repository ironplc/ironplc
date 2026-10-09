//! Tests for a call to a function of several inputs of one type assigned to
//! a target: its result has the type every input widens to, so a target
//! narrower than the widest input is a mismatch (#2127).

use ironplc_problems::Problem;
use spec_test_macro::spec_test;

rule_err!(
    #[spec_test(REQ_IC_analyzer_078)]
    apply_when_max_of_narrow_and_wide_assigned_to_narrow_then_return_type_mismatch,
    "PROGRAM main VAR i : INT; l : LINT; i2 : INT; END_VAR i2 := MAX(i, l); END_PROGRAM",
    [Problem::FunctionCallReturnTypeMismatch]
);

rule_ok!(
    apply_when_max_of_narrow_and_wide_assigned_to_wide_then_ok,
    "PROGRAM main VAR i : INT; l : LINT; l2 : LINT; END_VAR l2 := MAX(i, l); END_PROGRAM"
);
