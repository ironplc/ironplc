//! Tests for the enumerated values a structure or array initializer gives a
//! field or an element: `s : S := (c := Color#red)` and
//! `a : ARRAY[1..2] OF Color := [red, Color#green]`. Each value is checked
//! against the type of its field or element, as a variable's own initial
//! value is checked against the variable's type.

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
  E_Twin : (red, green);
  E_Paint : E_Color;
  MYINT : INT;
  S2 : STRUCT c : E_Color; END_STRUCT;
  S_Inline : STRUCT e : (A, B); END_STRUCT;
  S_Nested : STRUCT s : S2; a : ARRAY[1..2] OF E_Color; END_STRUCT;
  Row : ARRAY[1..2] OF E_Color;
END_TYPE

FUNCTION_BLOCK FB_Color
VAR_INPUT
  c : E_Color;
END_VAR
END_FUNCTION_BLOCK";

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

rule_err!(
    apply_when_structure_and_array_initializers_name_other_enumeration_or_non_member_then_reports_each,
    "
TYPE E_Color : (red, green); END_TYPE
TYPE E_Size : (small, large); END_TYPE
TYPE S2 : STRUCT c : E_Color; END_STRUCT; END_TYPE

PROGRAM main
VAR
    s2 : S2 := (c := E_Size#red);
    s3 : S2 := (c := E_Size#small);
    a  : ARRAY[1..2] OF E_Color := [E_Size#red, green];
    b  : ARRAY[1..2] OF E_Color := [E_Size#small, green];
    d  : ARRAY[1..2] OF E_Color := [blue, green];
END_VAR
END_PROGRAM",
    [
        Problem::EnumValueTypeMismatch,
        Problem::EnumValueTypeMismatch,
        Problem::EnumValueTypeMismatch,
        Problem::EnumValueTypeMismatch,
        Problem::EnumValueNotDefined
    ]
);

rule_err_at!(
    apply_when_structure_field_written_with_other_enumeration_then_labels_written_type,
    &in_variable("s : S2 := (c := E_Size#red);"),
    Problem::EnumValueTypeMismatch,
    "E_Size"
);

rule_err_at!(
    apply_when_array_element_not_a_member_then_labels_value,
    &in_variable("a : ARRAY[1..2] OF E_Color := [green, blue];"),
    Problem::EnumValueNotDefined,
    "blue"
);

rule_err!(
    apply_when_repetitions_written_with_other_enumeration_and_not_a_member_then_reports_each_once,
    &in_variable("a : ARRAY[1..4] OF E_Color := [2(E_Size#red), 2(blue)];"),
    [Problem::EnumValueTypeMismatch, Problem::EnumValueNotDefined]
);

#[rstest]
#[case::structure_value_of_neither("s : S2 := (c := E_Size#red);")]
// The value is a member of the written type, which is not the field's type.
#[case::structure_value_of_written_type("s : S2 := (c := E_Size#small);")]
// Two enumerations that list the same members are two enumerations.
#[case::structure_same_members("s : S2 := (c := E_Twin#red);")]
#[case::nested_structure("n : S_Nested := (s := (c := E_Size#red));")]
#[case::array_in_structure("n : S_Nested := (a := [green, E_Size#red]);")]
#[case::function_block("f : FB_Color := (c := E_Size#red);")]
// An inline enumeration has no name for a value to be written with.
#[case::inline_enumeration_field("x : S_Inline := (e := E_Size#small);")]
#[case::array_value_of_neither("a : ARRAY[1..2] OF E_Color := [E_Size#red, green];")]
#[case::array_value_of_written_type("a : ARRAY[1..2] OF E_Color := [E_Size#small, green];")]
#[case::array_repetition("a : ARRAY[1..4] OF E_Color := [2(E_Size#red), 2(green)];")]
#[case::named_array("r : Row := [green, E_Size#red];")]
// An array of arrays lists the elements of its innermost arrays.
#[case::array_of_arrays("g : ARRAY[1..2] OF Row := [red, green, E_Size#red, green];")]
#[case::two_dimensions("g : ARRAY[1..2, 1..2] OF E_Color := [red, green, E_Size#red, green];")]
fn apply_when_initializer_value_written_with_other_enumeration_then_enum_value_type_mismatch(
    #[case] declaration: &str,
) {
    assert_eq!(
        codes_of(&in_variable(declaration)),
        codes(&[Problem::EnumValueTypeMismatch])
    );
}

#[rstest]
#[case::structure_elementary_alias("s : S2 := (c := MYINT#red);")]
#[case::structure_structure("s : S2 := (c := S2#red);")]
#[case::structure_undeclared("s : S2 := (c := NOPE#red);")]
#[case::array_elementary_alias("a : ARRAY[1..2] OF E_Color := [MYINT#red, green];")]
#[case::array_undeclared("a : ARRAY[1..2] OF E_Color := [NOPE#red, green];")]
fn apply_when_initializer_value_written_with_type_not_an_enumeration_then_enum_not_declared(
    #[case] declaration: &str,
) {
    assert_eq!(
        codes_of(&in_variable(declaration)),
        codes(&[Problem::EnumNotDeclared])
    );
}

#[rstest]
#[case::structure_value_of_other_enumeration("s : S2 := (c := small);")]
#[case::structure_written_with_field_type("s : S2 := (c := E_Color#blue);")]
#[case::nested_structure("n : S_Nested := (s := (c := small));")]
#[case::inline_enumeration_field("x : S_Inline := (e := green);")]
#[case::function_block("f : FB_Color := (c := large);")]
#[case::array_undeclared_value("a : ARRAY[1..2] OF E_Color := [blue, green];")]
#[case::array_value_of_other_enumeration("a : ARRAY[1..2] OF E_Color := [small, green];")]
#[case::array_repetition("a : ARRAY[1..2] OF E_Color := [2(blue)];")]
#[case::named_array("r : Row := [green, small];")]
fn apply_when_initializer_value_not_a_member_then_enum_value_not_defined(
    #[case] declaration: &str,
) {
    assert_eq!(
        codes_of(&in_variable(declaration)),
        codes(&[Problem::EnumValueNotDefined])
    );
}

#[rstest]
#[case::structure_bare_value("s : S2 := (c := red);")]
#[case::structure_written_with_field_type("s : S2 := (c := E_Color#green);")]
#[case::structure_written_with_alias("s : S2 := (c := E_Paint#red);")]
#[case::nested_structure("n : S_Nested := (s := (c := green), a := [E_Paint#red, green]);")]
#[case::inline_enumeration_field("x : S_Inline := (e := B);")]
#[case::function_block("f : FB_Color := (c := E_Color#green);")]
#[case::array("a : ARRAY[1..3] OF E_Color := [red, E_Color#green, E_Paint#red];")]
#[case::array_repetition("a : ARRAY[1..4] OF E_Color := [2(E_Paint#green), 2()];")]
#[case::named_array("r : Row := [green, E_Color#red];")]
#[case::array_of_arrays("g : ARRAY[1..2] OF Row := [red, green, E_Paint#red, green];")]
fn apply_when_initializer_value_of_field_or_element_enumeration_then_ok(#[case] declaration: &str) {
    assert_eq!(codes_of(&in_variable(declaration)), codes(&[]));
}

#[rstest]
#[case::structure_field_structure_default("S4 : STRUCT s : S2 := (c := E_Size#red); END_STRUCT;")]
#[case::structure_field_array_default(
    "S5 : STRUCT a : ARRAY[1..2] OF E_Color := [E_Size#red, green]; END_STRUCT;"
)]
#[case::array_type_default("Pair : ARRAY[1..2] OF E_Color := [E_Size#red, green];")]
#[case::array_alias_default("RowDefault : Row := [E_Size#red, green];")]
fn apply_when_type_declaration_initializer_written_with_other_enumeration_then_enum_value_type_mismatch(
    #[case] declaration: &str,
) {
    assert_eq!(
        codes_of(&in_type(declaration)),
        codes(&[Problem::EnumValueTypeMismatch])
    );
}

// An array initializer for a type that is not an array is reported by
// `rule_array_initializer_type`, so its values have no element type here.
rule_ok!(
    apply_when_array_initializer_for_enumeration_then_ok,
    &in_variable("x : E_Color := [red, blue];")
);
