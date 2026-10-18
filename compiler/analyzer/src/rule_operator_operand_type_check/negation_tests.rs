//! Tests for negation, `-x`: a time of day, a date or a date and time has no
//! negative, in any dialect (P4049); a number and a duration do.

use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_problems::Problem;
use rstest::rstest;

use crate::test_helpers::rule_codes;

/// Edition 3, which has the long time and date types.
fn ed3() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
}

/// A program with a variable `x` of `declared_type` that it negates.
fn negating(declared_type: &str) -> String {
    format!(
        "
PROGRAM main
VAR
    x : {declared_type};
END_VAR
    x := -x;
END_PROGRAM"
    )
}

fn codes_for(program: &str, options: &CompilerOptions) -> Vec<String> {
    rule_codes(super::apply, program, options)
}

fn p4049() -> Vec<String> {
    vec![Problem::OperatorOperandTypeMismatch.code().to_string()]
}

#[rstest]
#[case::tod("TOD")]
#[case::time_of_day("TIME_OF_DAY")]
#[case::date("DATE")]
#[case::dt("DT")]
#[case::date_and_time("DATE_AND_TIME")]
#[case::ltod("LTOD")]
#[case::ldate("LDATE")]
#[case::ldt("LDT")]
fn apply_when_time_of_day_or_date_negated_then_p4049(#[case] declared_type: &str) {
    assert_eq!(codes_for(&negating(declared_type), &ed3()), p4049());
}

#[test]
fn apply_when_time_of_day_negated_in_every_dialect_then_p4049() {
    for dialect in Dialect::ALL {
        let options = CompilerOptions::from_dialect(*dialect);
        assert_eq!(
            codes_for(&negating("TOD"), &options),
            p4049(),
            "{dialect:?}"
        );
    }
}

#[rstest]
#[case::dint("DINT")]
#[case::lint("LINT")]
#[case::real("REAL")]
#[case::lreal("LREAL")]
#[case::time("TIME")]
#[case::ltime("LTIME")]
fn apply_when_number_or_duration_negated_then_ok(#[case] declared_type: &str) {
    assert!(codes_for(&negating(declared_type), &ed3()).is_empty());
}

rule_err!(
    /// A typed literal is a time of day as a variable is, and a negation
    /// inside an expression is checked as one on its own is.
    apply_when_date_literal_negated_inside_expression_then_p4049,
    "
PROGRAM main
VAR
    d : DATE;
    t : TIME;
END_VAR
    t := d - -D#2024-01-01;
END_PROGRAM",
    [Problem::OperatorOperandTypeMismatch]
);

rule_err_at!(
    apply_when_time_of_day_negated_then_p4049_at_operand,
    "
PROGRAM main
VAR
    t : TOD;
END_VAR
    t := -TOD#08:00:00;
END_PROGRAM",
    Problem::OperatorOperandTypeMismatch,
    "TOD#08:00:00"
);
