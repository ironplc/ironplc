//! Tests for a constant passed to a method: each argument is checked against
//! the parameter it binds to, as an argument to a function or function block
//! is. Methods are a CODESYS and TwinCAT extension, so each case runs in both
//! dialects.

use crate::test_helpers::{codes, rule_codes_after_conversions};
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_problems::Problem;
use rstest::rstest;

/// No problems.
const OK: &[Problem] = &[];
/// One constant outside its type's range.
const OVERFLOW: &[Problem] = &[Problem::ConstantOverflow];
const OVERFLOW_AND_REAL: &[Problem] = &[Problem::ConstantOverflow, Problem::RealLiteralOutOfRange];

/// A block `BASE` with methods taking a `USINT` and a `REAL`, returning a
/// `USINT`, and taking a `VAR_IN_OUT` beside a `USINT`, and a block
/// `DERIVED` that inherits them.
const METHOD_TYPES: &str = "
FUNCTION_BLOCK BASE
METHOD Put
VAR_INPUT i : USINT; r : REAL; END_VAR
END_METHOD
METHOD Get : USINT
VAR_INPUT i : USINT; END_VAR
Get := i;
END_METHOD
METHOD Swap
VAR_IN_OUT io : USINT; END_VAR
VAR_INPUT i : USINT; END_VAR
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK DERIVED EXTENDS BASE
END_FUNCTION_BLOCK
";

#[rstest]
#[case::named("b.Put(i := 300, r := 1.0E300);\n", OVERFLOW_AND_REAL)]
#[case::positional("b.Put(300, 1.0E300);\n", OVERFLOW_AND_REAL)]
#[case::in_range("b.Put(255, 1.0);\n", OK)]
#[case::folded("b.Put(255 + 1, 1.0);\n", OVERFLOW)]
#[case::inherited("d.Put(i := 300, r := 1.0);\n", OVERFLOW)]
#[case::expression("y := b.Get(300);\n", OVERFLOW)]
#[case::named_in_out("b.Swap(io := y, i := 300);\n", OVERFLOW)]
fn apply_when_method_argument_out_of_range_then_err(
    #[case] body: &str,
    #[case] expected: &[Problem],
    #[values(Dialect::Codesys, Dialect::TwinCat)] dialect: Dialect,
) {
    let program = format!(
        "{METHOD_TYPES}PROGRAM main
VAR b : BASE; d : DERIVED; y : USINT; END_VAR
{body}END_PROGRAM
"
    );

    assert_eq!(
        rule_codes_after_conversions(
            super::apply,
            &program,
            &CompilerOptions::from_dialect(dialect)
        ),
        codes(expected)
    );
}

/// An instance brought in with `VAR_EXTERNAL` is resolved through the
/// configuration's `VAR_GLOBAL`.
#[rstest]
fn apply_when_method_called_on_external_instance_then_err(
    #[values(Dialect::Codesys, Dialect::TwinCat)] dialect: Dialect,
) {
    let program = format!(
        "{METHOD_TYPES}PROGRAM main
VAR_EXTERNAL g : BASE; END_VAR
g.Put(i := 300, r := 1.0);
END_PROGRAM

CONFIGURATION config
VAR_GLOBAL g : BASE; END_VAR
RESOURCE res ON PLC
TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM plc_task_instance WITH plc_task : main;
END_RESOURCE
END_CONFIGURATION
"
    );

    assert_eq!(
        rule_codes_after_conversions(
            super::apply,
            &program,
            &CompilerOptions::from_dialect(dialect)
        ),
        codes(OVERFLOW)
    );
}
