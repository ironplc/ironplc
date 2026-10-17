//! End-to-end tests for an output of a function block call stored in a
//! target of another type: the output is converted by its own type, as
//! `x := fb.q` converts it, so an unsigned output is not sign-extended and a
//! `REAL` output is not read as an `LREAL`'s bits (#2125).

use spec_test_macro::spec_test;

e2e_f64!(
    #[spec_test(REQ_IC_codegen_022)]
    end_to_end_when_real_output_stored_in_lreal_then_converted,
    "FUNCTION_BLOCK FB
     VAR_INPUT x : REAL; END_VAR
     VAR_OUTPUT q : REAL; END_VAR
       q := x;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR f : FB; lr : LREAL; END_VAR
       f(x := 1.5, q => lr);
     END_PROGRAM",
    &[("lr", 1.5)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_022)]
    end_to_end_when_unsigned_output_stored_in_lint_then_zero_extended,
    "FUNCTION_BLOCK FB
     VAR_INPUT u : UDINT; END_VAR
     VAR_OUTPUT qu : UDINT; END_VAR
       qu := u;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR f : FB; lu : LINT; END_VAR
       f(u := 4000000000, qu => lu);
     END_PROGRAM",
    &[("lu", 4_000_000_000)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_022)]
    end_to_end_when_signed_output_stored_in_lint_element_then_sign_extended,
    "FUNCTION_BLOCK FB
     VAR_OUTPUT qd : DINT; END_VAR
       qd := -5;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR f : FB; a : ARRAY[1..2] OF LINT; l : LINT; END_VAR
       f(qd => a[2]);
       l := a[2];
     END_PROGRAM",
    &[("l", -5)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_022)]
    end_to_end_when_standard_counter_output_below_zero_stored_in_lint_then_sign_extended,
    "PROGRAM main
     VAR c : CTD; l : LINT; read : LINT; END_VAR
       c(CD := TRUE, CV => l);
       read := c.CV;
     END_PROGRAM",
    &[("l", -1), ("read", -1)],
);
