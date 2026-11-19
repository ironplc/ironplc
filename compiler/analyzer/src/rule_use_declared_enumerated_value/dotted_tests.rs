//! Tests for enumerated values written `Type.value`. The period spelling
//! names the same value as `Type#value`, so each gets the checks the `#`
//! spelling gets: `P2004` when the written type is not an enumeration,
//! `P2045` when it is another enumeration, and `P2006` when the value is not
//! a member.
//!
//! Whether the period spelling is allowed at all is
//! `rule_dotted_enum_value_allowed`'s to report, not this rule's.

use crate::test_helpers::{codes, rule_codes};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;

/// The types the programs below use, declared after the program so that a
/// label points at the first occurrence of a name in the program.
const TYPES: &str = "
TYPE
  E_Color : (red, green);
  E_Size : (small, large);
  E_Paint : E_Color;
  MYINT : INT;
END_TYPE";

/// A program declaring the variable `declaration`.
fn in_variable(declaration: &str) -> String {
    format!(
        "
PROGRAM main
VAR
  {declaration}
END_VAR
END_PROGRAM
{TYPES}"
    )
}

/// A program whose `CASE` on a selector of the type `selector` has the
/// label `label`.
fn in_case(selector: &str, label: &str) -> String {
    format!(
        "
PROGRAM main
VAR
  s : {selector};
  out : INT;
END_VAR
  CASE s OF
    {label}: out := 1;
  END_CASE;
END_PROGRAM
{TYPES}"
    )
}

/// A `TYPE` declaration `declaration`, ahead of a program that uses nothing.
fn in_type(declaration: &str) -> String {
    format!(
        "
TYPE
  {declaration}
END_TYPE

PROGRAM main
END_PROGRAM
{TYPES}"
    )
}

fn codes_of(program: &str) -> Vec<String> {
    rule_codes(super::apply, program, &CompilerOptions::default())
}

#[rstest]
#[case::variable(in_variable("c : E_Color := E_Color.green;"))]
#[case::variable_written_with_alias(in_variable("c : E_Color := E_Paint.red;"))]
#[case::alias_variable(in_variable("p : E_Paint := E_Color.red;"))]
#[case::array_element(in_variable("a : ARRAY[1..2] OF E_Color := [E_Color.red, E_Paint.green];"))]
#[case::case_label(in_case("E_Color", "E_Color.red, E_Paint.green"))]
#[case::type_default(in_type("E_Mode : (idle, run) := E_Mode.run;"))]
fn apply_when_dotted_value_of_its_enumeration_then_ok(#[case] program: String) {
    assert_eq!(codes_of(&program), codes(&[]));
}

#[rstest]
#[case::variable(in_variable("c : E_Color := E_Size.small;"))]
#[case::array_element(in_variable("a : ARRAY[1..2] OF E_Color := [E_Size.small, green];"))]
#[case::case_label(in_case("E_Color", "E_Size.small"))]
// A label written with its type names an enumeration, which an integer
// selector has no values of.
#[case::case_label_on_integer(in_case("INT", "E_Color.red"))]
#[case::type_default(in_type("E_Mode : (idle, run) := E_Color.red;"))]
fn apply_when_dotted_value_written_with_other_enumeration_then_enum_value_type_mismatch(
    #[case] program: String,
) {
    assert_eq!(codes_of(&program), codes(&[Problem::EnumValueTypeMismatch]));
}

#[rstest]
#[case::variable(in_variable("c : E_Color := E_Color.blue;"))]
#[case::array_element(in_variable("a : ARRAY[1..2] OF E_Color := [E_Color.blue, green];"))]
#[case::case_label(in_case("E_Color", "E_Color.blue"))]
fn apply_when_dotted_value_not_a_member_then_enum_value_not_defined(#[case] program: String) {
    assert_eq!(codes_of(&program), codes(&[Problem::EnumValueNotDefined]));
}

#[rstest]
#[case::variable_undeclared(in_variable("c : E_Color := E_Nope.red;"))]
#[case::case_label_undeclared(in_case("E_Color", "E_Nope.red"))]
#[case::case_label_elementary_alias(in_case("E_Color", "MYINT.red"))]
#[case::array_element_undeclared(in_variable(
    "a : ARRAY[1..2] OF E_Color := [E_Nope.red, green];"
))]
fn apply_when_dotted_value_written_with_type_not_an_enumeration_then_enum_not_declared(
    #[case] program: String,
) {
    assert_eq!(codes_of(&program), codes(&[Problem::EnumNotDeclared]));
}
