//! Tests for where an assignment type mismatch (P4035) points: the label
//! underlines the whole assigned value as written, so a multi-element
//! variable is underlined up to its last element. See
//! https://github.com/ironplc/ironplc/issues/1976.

use crate::test_helpers::parse_and_resolve_types_with_context;
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;

/// Checks `i := <value>;` in a program that declares one variable of each
/// kind the tests use, and returns the source text the single P4035 label
/// underlines.
fn underlined_value(value: &str) -> String {
    let program = format!(
        "
TYPE
  INNER : STRUCT y : BOOL; END_STRUCT;
  S : STRUCT x : BOOL; inner : INNER; END_STRUCT;
END_TYPE

PROGRAM main
VAR
  i : INT;
  s : S;
  w : WORD;
  t : TON;
END_VAR
  i := {value};
END_PROGRAM"
    );
    let (library, context) = parse_and_resolve_types_with_context(&program);
    let errors = super::apply(&library, &context, &CompilerOptions::default()).unwrap_err();
    assert_eq!(errors.len(), 1, "expected one diagnostic, got {errors:?}");
    assert_eq!(errors[0].code, Problem::AssignmentTypeMismatch.code());

    let location = &errors[0].primary.location;
    program[location.start..location.end].to_string()
}

#[rstest]
#[case::structured("s.x")]
#[case::chained_structured("s.inner.y")]
#[case::function_block_member("t.Q")]
#[case::bit_access("w.3")]
fn apply_when_assigned_value_is_multi_element_variable_then_label_covers_the_variable(
    #[case] value: &str,
) {
    assert_eq!(value, underlined_value(value));
}
