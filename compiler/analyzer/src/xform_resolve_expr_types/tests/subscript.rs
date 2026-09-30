//! Tests for the type of a subscripted variable: an element of an array, of
//! an array of arrays, or of an array field.

use super::{collect_assignment_types, run_pass_with_options, type_name_upper};
use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

/// The type name of the value `x := {value};` assigns, in a program that
/// declares a variable of each array shape the tests read.
fn value_type(value: &str) -> Option<String> {
    let program = format!(
        "
TYPE
  Color : (RED, GREEN);
  Row : ARRAY[1..3] OF DINT;
  Rows : ARRAY[1..2] OF Row;
  Colors : ARRAY[1..2] OF Color;
  Rec : STRUCT
    n : DINT;
    v : ARRAY[1..3] OF DINT;
    r : Row;
    t : ARRAY[1..2] OF Row;
    m : ARRAY[1..2, 1..3] OF DINT;
  END_STRUCT;
END_TYPE
PROGRAM main
VAR
  x : DINT;
  a : ARRAY[1..3] OF DINT;
  m : ARRAY[1..2, 1..3] OF DINT;
  row : Row;
  rows : ARRAY[1..2] OF Row;
  named_rows : Rows;
  grid : ARRAY[1..2, 1..2] OF Row;
  colors : ARRAY[1..2] OF Colors;
  rec : Rec;
  recs : ARRAY[1..2] OF Rec;
  pa : REF_TO ARRAY[1..3] OF DINT;
  prows : REF_TO Rows;
END_VAR
  x := {value};
END_PROGRAM
"
    );
    let resolved = run_pass_with_options(
        &program,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    type_name_upper(&collect_assignment_types(&resolved)[0])
}

#[rstest]
#[case::array("a[1]", "DINT")]
#[case::two_dimensions("m[1, 2]", "DINT")]
#[case::two_dimensions_one_bracket_each("m[1][2]", "DINT")]
#[case::named_array("row[2]", "DINT")]
#[case::array_of_arrays("rows[1][2]", "DINT")]
#[case::array_of_arrays_inner_array("rows[1]", "ROW")]
#[case::named_array_of_arrays("named_rows[2][3]", "DINT")]
#[case::named_array_of_arrays_inner_array("named_rows[2]", "ROW")]
#[case::two_dimensions_of_arrays("grid[1, 2][3]", "DINT")]
#[case::two_dimensions_of_arrays_inner_array("grid[1, 2]", "ROW")]
#[case::two_dimensions_of_arrays_one_bracket_each("grid[1][2][3]", "DINT")]
#[case::array_of_enumeration_arrays("colors[1][2]", "COLOR")]
#[case::structure_field("rec.v[2]", "DINT")]
#[case::structure_field_named_array("rec.r[2]", "DINT")]
#[case::structure_field_array_of_arrays("rec.t[1][2]", "DINT")]
#[case::structure_field_two_dimensions_one_bracket_each("rec.m[1][2]", "DINT")]
#[case::field_of_array_element("recs[2].n", "DINT")]
#[case::array_field_of_array_element("recs[2].v[3]", "DINT")]
#[case::array_of_arrays_field_of_array_element("recs[2].t[1][3]", "DINT")]
#[case::dereference("pa^[2]", "DINT")]
#[case::reference("pa[2]", "DINT")]
#[case::dereference_array_of_arrays("prows^[1][2]", "DINT")]
fn apply_when_subscripted_variable_then_type_is_selected_element_type(
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(value_type(value), Some(expected.to_string()));
}

#[rstest]
#[case::two_dimensions_missing_subscript("m[1]")]
#[case::too_many_subscripts("a[1, 2]")]
#[case::subscripts_reach_past_inner_array("rows[1, 2]")]
#[case::subscript_of_element("a[1][2]")]
fn apply_when_subscripts_do_not_select_element_then_type_unresolved(#[case] value: &str) {
    assert_eq!(value_type(value), None);
}

#[test]
fn apply_when_array_of_arrays_element_compared_then_operands_resolve() {
    let program = "
TYPE
  Row : ARRAY[1..3] OF DINT;
END_TYPE
PROGRAM main
VAR
  rows : ARRAY[1..2] OF Row;
  b : BOOL;
  x : DINT;
END_VAR
  b := rows[1][2] > 3;
  x := rows[1][2] + rows[2][1];
END_PROGRAM
";
    let resolved = run_pass_with_options(program, &CompilerOptions::default());
    let types: Vec<Option<String>> = collect_assignment_types(&resolved)
        .iter()
        .map(type_name_upper)
        .collect();
    assert_eq!(
        types,
        vec![Some("BOOL".to_string()), Some("DINT".to_string())]
    );
}
