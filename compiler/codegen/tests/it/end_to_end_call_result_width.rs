//! End-to-end tests for a value whose type is the one its function or method
//! returns, or the one its referenced variable has, stored in a target of
//! another width: the value is converted by its own type, so an unsigned
//! result is not sign-extended and a `REAL` result is not read as an
//! `LREAL`'s bits (#2126).

use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

e2e_i64_with!(
    #[spec_test(REQ_IC_codegen_009)]
    end_to_end_when_unsigned_result_assigned_to_lint_then_zero_extended,
    CompilerOptions {
        allow_ref_to: true,
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    },
    "FUNCTION big : UDINT
     VAR_INPUT x : UDINT; END_VAR
       big := x;
     END_FUNCTION
     FUNCTION_BLOCK K
     METHOD Big : UDINT
       Big := 4000000000;
     END_METHOD
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR k : K; u : UDINT := 4000000000; p : REF_TO UDINT;
         l_call : LINT; l_method : LINT; l_deref : LINT; END_VAR
       p := REF(u);
       l_call := big(u);
       l_method := k.Big();
       l_deref := p^;
     END_PROGRAM",
    &[
        ("l_call", 4_000_000_000),
        ("l_method", 4_000_000_000),
        ("l_deref", 4_000_000_000)
    ],
);

e2e_f64!(
    #[spec_test(REQ_IC_codegen_009)]
    end_to_end_when_real_result_assigned_to_lreal_then_converted,
    "FUNCTION half : REAL
     VAR_INPUT x : REAL; END_VAR
       half := x / 2.0;
     END_FUNCTION
     PROGRAM main
     VAR lr : LREAL; END_VAR
       lr := half(3.0);
     END_PROGRAM",
    &[("lr", 1.5)],
);
