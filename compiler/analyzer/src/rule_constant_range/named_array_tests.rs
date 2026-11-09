//! Tests for the values of an array initializer written after the name of an
//! array type (`x : PAIR := [1, 300]`): each value is checked against the
//! element type the name resolves to, as for an array spelled in place.

use crate::test_helpers::{codes, rule_codes_after_conversions};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;

/// No problems.
const OK: &[Problem] = &[];
/// One constant outside its type's range.
const OVERFLOW: &[Problem] = &[Problem::ConstantOverflow];

/// An array type `PAIR` of `USINT`, and `ALIAS`, an alias of it.
const NAMED_ARRAY_TYPES: &str = "TYPE
PAIR : ARRAY[1..2] OF USINT;
ALIAS : PAIR;
END_TYPE
";

#[rstest]
#[case::variable("PROGRAM main VAR x : PAIR := [1, 300]; END_VAR END_PROGRAM", OVERFLOW)]
#[case::variable_repeated("PROGRAM main VAR x : PAIR := [2(300)]; END_VAR END_PROGRAM", OVERFLOW)]
#[case::variable_of_alias(
    "PROGRAM main VAR x : ALIAS := [1, 300]; END_VAR END_PROGRAM",
    OVERFLOW
)]
#[case::variable_in_range("PROGRAM main VAR x : PAIR := [0, 255]; END_VAR END_PROGRAM", OK)]
#[case::structure_field("TYPE S : STRUCT f : PAIR := [1, 300]; END_STRUCT; END_TYPE", OVERFLOW)]
#[case::type_alias_default("TYPE B : PAIR := [300]; END_TYPE", OVERFLOW)]
fn apply_when_named_array_initializer_out_of_range_then_err(
    #[case] declarations: &str,
    #[case] expected: &[Problem],
) {
    let program = format!("{NAMED_ARRAY_TYPES}{declarations}\n");

    assert_eq!(
        rule_codes_after_conversions(super::apply, &program, &CompilerOptions::default()),
        codes(expected)
    );
}
