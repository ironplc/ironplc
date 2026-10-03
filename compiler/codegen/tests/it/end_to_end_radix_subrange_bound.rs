//! End-to-end tests for subrange bounds written in hex, binary or octal
//! (a dialect extension): a CASE label range, an array dimension and a
//! subrange type behave as their decimal spelling does.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::assert_run_i32_with;

fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

/// Each selector value runs the arm whose radix range holds it.
#[rstest]
#[case::low_range(1, 1)]
#[case::low_range_end(15, 1)]
#[case::high_range(16, 2)]
#[case::high_range_end(255, 2)]
#[case::no_range(256, 0)]
fn end_to_end_when_case_label_range_has_radix_bounds_then_matches_range(
    #[case] selector: i32,
    #[case] expected: i32,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    y : DINT;
    x : DINT := {selector};
  END_VAR
  CASE x OF
    16#01..16#0F: y := 1;
    2#10000..8#377: y := 2;
  END_CASE;
END_PROGRAM
"
    );

    assert_run_i32_with(&source, &twincat(), &[(0, expected)]);
}

/// A radix range that fills an unsigned selector narrows by value, as a
/// radix label does.
#[test]
fn end_to_end_when_case_label_range_has_radix_bounds_and_unsigned_selector_then_matches() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
    x : UDINT := 4294967295;
  END_VAR
  CASE x OF
    16#F0000000..16#FFFFFFFF: y := 1;
  END_CASE;
END_PROGRAM
";

    assert_run_i32_with(source, &twincat(), &[(0, 1)]);
}

#[test]
fn end_to_end_when_array_dimension_has_radix_bounds_then_indexes_by_value() {
    let source = "
PROGRAM main
  VAR
    y : DINT;
    a : ARRAY[16#1..16#4] OF DINT;
  END_VAR
  a[16#4] := 7;
  a[1] := 3;
  y := a[4] * 10 + a[16#1];
END_PROGRAM
";

    assert_run_i32_with(source, &twincat(), &[(0, 73)]);
}

#[test]
fn end_to_end_when_subrange_type_has_radix_bounds_then_holds_value() {
    let source = "
TYPE
  Level : DINT (16#0..16#FF);
END_TYPE

PROGRAM main
  VAR
    y : DINT;
    level : Level := 16#C8;
  END_VAR
  y := level;
END_PROGRAM
";

    assert_run_i32_with(source, &twincat(), &[(0, 200)]);
}
