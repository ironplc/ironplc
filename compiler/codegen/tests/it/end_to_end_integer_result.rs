//! End-to-end tests for `TRUNC`, `BCD_TO_INT` and `SIZEOF`, whose integer
//! result has the type the analyzer recorded for the call: its context's
//! when that is an integer type, else a `DINT` converted to it.

use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

e2e_f32!(
    #[spec_test(REQ_IC_codegen_011)]
    end_to_end_when_trunc_assigned_to_real_then_truncates,
    "FUNCTION keep : REAL
     VAR_INPUT x : REAL; END_VAR
       keep := x;
     END_FUNCTION
     PROGRAM main
     VAR r : REAL := 2.75; assigned : REAL; passed : REAL; END_VAR
       assigned := TRUNC(r);
       passed := keep(TRUNC(r));
     END_PROGRAM",
    &[("assigned", 2.0), ("passed", 2.0)],
);

e2e_f64!(
    #[spec_test(REQ_IC_codegen_011)]
    end_to_end_when_trunc_assigned_to_lreal_then_truncates,
    "PROGRAM main
     VAR lr : LREAL := -2.75; t : LREAL; END_VAR
       t := TRUNC(lr);
     END_PROGRAM",
    &[("t", -2.0)],
);

e2e_i64_with!(
    #[spec_test(REQ_IC_codegen_011)]
    end_to_end_when_integer_result_assigned_to_lint_then_computes_at_64_bits,
    CompilerOptions {
        allow_sizeof: true,
        ..CompilerOptions::default()
    },
    "PROGRAM main
     VAR lr : LREAL := 5000000000.5; bcd : LWORD := 16#1234567890;
         truncated : LINT; decoded : LINT; size : LINT; END_VAR
       truncated := TRUNC(lr);
       decoded := BCD_TO_INT(bcd);
       size := SIZEOF(lr);
     END_PROGRAM",
    &[
        ("truncated", 5_000_000_000),
        ("decoded", 1_234_567_890),
        ("size", 8)
    ],
);
