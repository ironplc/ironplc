//! End-to-end tests for the result of a standard function whose signature
//! fixes its type, stored in a target of another width: the result is
//! converted by its own type rather than its bits stored as the target's.

use ironplc_parser::options::{CompilerOptions, Dialect};
use spec_test_macro::spec_test;

use crate::common::{assert_run_with, date, datetime};

e2e_f64!(
    #[spec_test(REQ_IC_codegen_017)]
    end_to_end_when_conversion_to_real_assigned_to_lreal_then_widened,
    "PROGRAM main
     VAR i : INT := 3; d : DINT := -5; from_int : LREAL; from_dint : LREAL; END_VAR
       from_int := INT_TO_REAL(i);
       from_dint := DINT_TO_REAL(d);
     END_PROGRAM",
    &[("from_int", 3.0), ("from_dint", -5.0)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_017)]
    end_to_end_when_unsigned_conversion_or_len_assigned_to_lint_then_widened,
    "PROGRAM main
     VAR d : DINT := -5; s : STRING := 'hello'; unsigned : LINT; length : LINT; END_VAR
       unsigned := DINT_TO_UDINT(d);
       length := LEN(s);
     END_PROGRAM",
    &[("unsigned", 4_294_967_291), ("length", 5)],
);

#[spec_test(REQ_IC_codegen_017)]
#[test]
fn end_to_end_when_date_results_after_2038_assigned_to_long_types_then_keep_value() {
    let program = |target: &str, value: &str| {
        format!(
            "PROGRAM main
             VAR dt1 : DT := DT#2100-01-01-10:00:00; t : TIME := T#1h; result : {target}; END_VAR
               result := {value};
             END_PROGRAM"
        )
    };
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    assert_run_with(
        &program("LDT", "SUB_DT_TIME(dt1, t)"),
        &options,
        &[("result", datetime!(2100-01-01 9:00))],
    );
    assert_run_with(
        &program("LDATE", "DT_TO_DATE(dt1)"),
        &options,
        &[("result", date!(2100 - 01 - 01))],
    );
}
