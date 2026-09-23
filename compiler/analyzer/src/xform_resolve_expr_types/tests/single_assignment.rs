//! The "single assignment, resolves to type T" case table.

use super::*;

/// Parameterized tests for the "single assignment, resolves to type T" shape.
///
/// Each case is a complete IEC 61131-3 program containing exactly one
/// top-level assignment; the test runs the expression-type-resolution
/// pipeline and asserts the RHS resolves to the expected type name.
/// This replaces 27 near-identical hand-written tests.
#[rstest]
#[case::simple_int_var(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := x;
END_FUNCTION_BLOCK",
    "INT"
)]
#[case::type_alias_to_elementary(
    "
TYPE
    MyByte : BYTE := 0;
END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    x : MyByte;
    y : BYTE;
END_VAR
    y := x;
END_FUNCTION_BLOCK",
    "BYTE"
)]
#[case::bool_literal(
    "
FUNCTION_BLOCK FB_TEST
VAR
    y : BOOL;
END_VAR
    y := TRUE;
END_FUNCTION_BLOCK",
    "BOOL"
)]
#[case::typed_integer_literal(
    "
FUNCTION_BLOCK FB_TEST
VAR
    y : INT;
END_VAR
    y := INT#42;
END_FUNCTION_BLOCK",
    "INT"
)]
#[case::comparison_resolves_bool(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
END_VAR
    y := x > 0;
END_FUNCTION_BLOCK",
    "BOOL"
)]
#[case::binary_op_inherits_operand_type(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := x + x;
END_FUNCTION_BLOCK",
    "INT"
)]
#[case::unary_op_inherits_operand_type(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := -x;
END_FUNCTION_BLOCK",
    "INT"
)]
// AND has generic return type ANY_BIT; the function form resolves to the
// operand type like the operator form does (#1567).
#[case::and_function_on_word_resolves_operand_type(
    "
PROGRAM test
  VAR
    a : WORD;
    b : WORD;
    result : WORD;
  END_VAR
    result := AND(a, b);
END_PROGRAM",
    "WORD"
)]
// ABS has generic return type ANY_NUM; should resolve to concrete input type.
#[case::function_call_resolves_return_type(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := ABS(x);
END_FUNCTION_BLOCK",
    "INT"
)]
// SHR has generic return type ANY_BIT; ABS(a) resolves to DINT,
// so the outer SHR should also resolve to DINT.
#[case::nested_function_call_resolves_concrete_type(
    "
PROGRAM test
  VAR
    a : DINT;
    result : DINT;
  END_VAR
    result := SHR(ABS(a), 1);
END_PROGRAM",
    "DINT"
)]
// The RHS expression type reflects the expression itself, not the target.
// Here TRUE is BOOL even though the target y is INT.
#[case::bool_assigned_to_int_var_rhs_resolves_bool(
    "
FUNCTION_BLOCK FB_TEST
VAR
    y : INT;
END_VAR
    y := TRUE;
END_FUNCTION_BLOCK",
    "BOOL"
)]
// The expression type is determined by the expression, not the target.
#[case::int_assigned_to_bool_var_rhs_resolves_int(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
END_VAR
    y := x;
END_FUNCTION_BLOCK",
    "INT"
)]
// In x + y where x is DINT and y is INT, the result inherits the left operand type.
#[case::mixed_type_binary_op_inherits_left_operand_type(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : DINT;
    y : INT;
    result : DINT;
END_VAR
    result := x + y;
END_FUNCTION_BLOCK",
    "DINT"
)]
// In 5 + x where x is INT, the result should be INT (concrete wins over ANY_INT).
#[case::binary_op_literal_plus_concrete(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    result : INT;
END_VAR
    result := 5 + x;
END_FUNCTION_BLOCK",
    "INT"
)]
// In x + 5 where x is INT, the result should be INT (left is concrete).
#[case::binary_op_concrete_plus_literal(
    "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    result : INT;
END_VAR
    result := x + 5;
END_FUNCTION_BLOCK",
    "INT"
)]
// In 5 + 10, both are ANY_INT, so the result is ANY_INT.
#[case::binary_op_two_literals_resolves_any_int(
    "
FUNCTION_BLOCK FB_TEST
VAR
    result : DINT;
END_VAR
    result := 5 + 10;
END_FUNCTION_BLOCK",
    "ANY_INT"
)]
#[case::real_literal_without_type_resolves_any_real(
    "
FUNCTION_BLOCK FB_TEST
VAR
    y : REAL;
END_VAR
    y := 3.14;
END_FUNCTION_BLOCK",
    "ANY_REAL"
)]
// A subrange variable like INT(-100..100) should resolve to INT.
#[case::subrange_var_resolves_base_type(
    "
FUNCTION_BLOCK FB_TEST
VAR_IN_OUT
    x : INT(-100..100);
END_VAR
VAR
    y : INT;
END_VAR
    y := x;
END_FUNCTION_BLOCK",
    "INT"
)]
#[case::string_literal(
    "
FUNCTION_BLOCK FB_TEST
VAR
    s : STRING;
END_VAR
    s := 'hello';
END_FUNCTION_BLOCK",
    "STRING"
)]
// The quotes are the type: a double-quoted literal is a WSTRING, so the
// wide target accepts it. Typing it STRING made this assignment P4035.
#[case::wstring_literal(
    "
FUNCTION_BLOCK FB_TEST
VAR
    s : WSTRING;
END_VAR
    s := \"hello\";
END_FUNCTION_BLOCK",
    "WSTRING"
)]
#[case::untyped_integer_literal_resolves_any_int(
    "
FUNCTION_BLOCK FB_TEST
VAR
    y : DINT;
END_VAR
    y := 42;
END_FUNCTION_BLOCK",
    "ANY_INT"
)]
#[case::time_literal(
    "
FUNCTION_BLOCK FB_TEST
VAR
    t : TIME;
END_VAR
    t := T#5s;
END_FUNCTION_BLOCK",
    "TIME"
)]
#[case::sel_resolves_value_type_not_selector(
    "
PROGRAM test
  VAR
    g : BOOL;
    a : INT;
    b : INT;
    result : INT;
  END_VAR
    result := SEL(g, a, b);
END_PROGRAM",
    "INT"
)]
#[case::mux_resolves_value_type_not_selector(
    "
PROGRAM test
  VAR
    k : INT;
    a : DINT;
    b : DINT;
    result : DINT;
  END_VAR
    result := MUX(k, a, b);
END_PROGRAM",
    "DINT"
)]
#[case::sel_nested_in_function(
    "
PROGRAM test
  VAR
    g : BOOL;
    a : INT;
    b : INT;
    result : INT;
  END_VAR
    result := ABS(SEL(g, a, b));
END_PROGRAM",
    "INT"
)]
#[case::named_array_subscript(
    "
TYPE MyArr : ARRAY[0..10] OF INT; END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    arr : MyArr;
    result : INT;
END_VAR
    result := arr[0];
END_FUNCTION_BLOCK",
    "INT"
)]
#[case::inline_array_subscript(
    "
FUNCTION_BLOCK FB_TEST
VAR
    arr : ARRAY[0..10] OF DINT;
    result : DINT;
END_VAR
    result := arr[0];
END_FUNCTION_BLOCK",
    "DINT"
)]
// Regression for the `compile_expr.rs#L32` TODO that fired when
// `struct.field[i, j]` was used in a STRING comparison: the analyzer
// previously left `resolved_type` unset for array subscripts whose
// base was a struct field.
#[case::struct_field_2d_string_array_subscript(
    "
TYPE MY_DATA : STRUCT
    DIRS : ARRAY[0..2, 0..15] OF STRING[3];
END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    data : MY_DATA;
    i : INT;
    j : INT;
    result : STRING[3];
END_VAR
    result := data.DIRS[i, j];
END_FUNCTION_BLOCK",
    "STRING"
)]
// Same fix must also cover numeric array fields, not just strings.
#[case::struct_field_int_array_subscript(
    "
TYPE MY_DATA : STRUCT
    values : ARRAY[0..9] OF DINT;
END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    data : MY_DATA;
    i : INT;
    result : DINT;
END_VAR
    result := data.values[i];
END_FUNCTION_BLOCK",
    "DINT"
)]
// A function block instance exposes its variables as named members, the
// same as a struct field. Leaving these unresolved made codegen fail with
// P9999 wherever the expression's own type was needed (issue #1375).
#[case::function_block_bool_output(
    "
FUNCTION_BLOCK FB_TEST
VAR
    timer : TON;
    result : BOOL;
END_VAR
    result := timer.Q;
END_FUNCTION_BLOCK",
    "BOOL"
)]
#[case::function_block_time_output(
    "
FUNCTION_BLOCK FB_TEST
VAR
    timer : TON;
    result : TIME;
END_VAR
    result := timer.ET;
END_FUNCTION_BLOCK",
    "TIME"
)]
#[case::function_block_input(
    "
FUNCTION_BLOCK FB_TEST
VAR
    timer : TON;
    result : BOOL;
END_VAR
    result := timer.IN;
END_FUNCTION_BLOCK",
    "BOOL"
)]
fn apply_when_single_assignment_then_resolves_expected_type(
    #[case] program: &str,
    #[case] expected: &str,
) {
    let result = run_pass(program);
    let types = collect_assignment_types(&result);
    assert_eq!(types.len(), 1);
    assert_type_eq(&types[0], expected);
}
