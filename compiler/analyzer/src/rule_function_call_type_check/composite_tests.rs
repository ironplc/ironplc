//! Tests for values whose type is not a scalar -- a whole array, a
//! structure, an enumeration -- used where a type is required. See
//! `value_type`.

use crate::test_helpers::parse_and_resolve_types_with_context;
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;

/// The problem codes the rule reports for `body` inside a program that
/// declares one variable of each kind the tests use.
fn problem_codes(body: &str) -> Vec<String> {
    let program = format!(
        "
TYPE
  REC : STRUCT x : DINT; END_STRUCT;
  ARR : ARRAY[1..2] OF DINT;
  COL : (RED, GREEN);
  BIG : LINT (-10..10);
END_TYPE

FUNCTION TAKES_DINT : DINT
VAR_INPUT x : DINT; END_VAR
  TAKES_DINT := x;
END_FUNCTION

FUNCTION TAKES_ARR : DINT
VAR_INPUT x : ARR; END_VAR
  TAKES_ARR := 1;
END_FUNCTION

FUNCTION TAKES_BIG : DINT
VAR_INPUT x : BIG; END_VAR
  TAKES_BIG := 1;
END_FUNCTION

PROGRAM main
VAR
  a : ARRAY[1..2] OF DINT;
  s : ARRAY[1..2] OF STRING[8];
  na : ARR;
  r : REC;
  e : (X, Y);
  ne : COL;
  q : BIG;
  n : DINT;
  l : LINT;
END_VAR
  {body}
END_PROGRAM"
    );
    let (library, context) = parse_and_resolve_types_with_context(&program);
    match super::apply(&library, &context, &CompilerOptions::default()) {
        Ok(()) => vec![],
        Err(diagnostics) => diagnostics.into_iter().map(|d| d.code).collect(),
    }
}

#[rstest]
#[case::len_of_inline_array("n := LEN(s);")]
#[case::abs_of_inline_array("n := ABS(a);")]
#[case::user_function_of_inline_array("n := TAKES_DINT(a);")]
#[case::len_of_named_array("n := LEN(na);")]
#[case::user_function_of_named_array("n := TAKES_DINT(na);")]
#[case::abs_of_inline_enumeration("n := ABS(e);")]
#[case::abs_of_structure("n := ABS(r);")]
#[case::structure_for_array_parameter("n := TAKES_ARR(r);")]
fn apply_when_composite_argument_for_other_type_then_p4026(#[case] body: &str) {
    assert!(problem_codes(body).contains(&Problem::FunctionCallArgTypeMismatch.code().to_string()));
}

#[rstest]
#[case::inline_array("n := a;")]
#[case::named_array("n := na;")]
#[case::structure("n := r;")]
#[case::inline_enumeration("n := e;")]
#[case::named_enumeration("n := ne;")]
fn apply_when_composite_assigned_to_elementary_then_p4035(#[case] body: &str) {
    assert_eq!(
        problem_codes(body),
        vec![Problem::AssignmentTypeMismatch.code().to_string()]
    );
}

#[rstest]
#[case::named_array_for_its_type("n := TAKES_ARR(na);")]
#[case::inline_array_of_same_shape("n := TAKES_ARR(a);")]
#[case::enumeration_value_to_its_type("ne := RED;")]
#[case::subrange_for_numeric_parameter("l := ABS(q);")]
#[case::subrange_for_its_type("n := TAKES_BIG(q);")]
#[case::subrange_widened("l := q;")]
fn apply_when_value_of_accepted_type_then_ok(#[case] body: &str) {
    assert_eq!(problem_codes(body), Vec::<String>::new());
}

#[test]
fn apply_when_subrange_narrowed_then_p4035() {
    assert_eq!(
        problem_codes("n := q;"),
        vec![Problem::AssignmentTypeMismatch.code().to_string()]
    );
}
