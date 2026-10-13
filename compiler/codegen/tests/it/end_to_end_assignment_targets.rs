//! End-to-end tests for an assignment to each shape of single-slot target: an
//! array element, an element of an array reached through a `REF_TO`, a
//! structure field, an element of an array in a structure (`s.arr[i]`), and a
//! field of an element of an array of structures (`a[i].f`).
//!
//! Each target is a `SINT` assigned `a + a` with `a = 100`, so the value
//! overflows and the store must truncate it: the target reads back -56, not
//! 200. The value is read back into a `DINT`, which does not truncate, so a
//! store that skipped the truncation would read back 200. `a + a` keeps the
//! value computed at run time; a constant would be truncated at compile time.
//!
//! A named variable and a `VAR_IN_OUT` parameter are covered by
//! `end_to_end_const_trunc.rs` and `end_to_end_user_function_in_out.rs`.

use crate::common::Snapshot;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;
use spec_test_macro::spec_test;

/// Declares a `SINT` target of each shape, runs `body`, and returns the
/// variables. `i` is 1, so a subscript is computed at run time.
fn run_on_targets(body: &str) -> Snapshot {
    let source = format!(
        "
TYPE ELEMENT : STRUCT f : SINT; END_STRUCT; END_TYPE
TYPE HOLDER : STRUCT f : SINT; arr : ARRAY[0..1] OF SINT; END_STRUCT; END_TYPE
PROGRAM main
  VAR
    i : DINT := 1;
    a : SINT := 100;
    arr : ARRAY[0..1] OF SINT;
    s : HOLDER;
    es : ARRAY[0..1] OF ELEMENT;
    r : DINT;
  END_VAR
  {body}
END_PROGRAM
"
    );
    Snapshot::run(&source, &CompilerOptions::default())
}

/// A value that overflows a narrow target is truncated to the target's width.
#[rstest]
#[case::array_element("arr[i]")]
#[case::struct_field("s.f")]
#[case::array_in_struct("s.arr[i]")]
#[case::field_of_array_of_struct("es[i].f")]
fn assignment_when_narrow_target_overflows_then_value_wraps_at_target_width(#[case] target: &str) {
    let snapshot = run_on_targets(&format!("{target} := a + a; r := {target};"));
    assert_eq!(snapshot.read_as::<i32>("r"), -56);
}

/// An element of an array reached through a `REF_TO ARRAY` is truncated to
/// the element's width, in the referenced array.
#[test]
fn assignment_when_narrow_element_through_reference_overflows_then_value_wraps_at_element_width() {
    let source = "
FUNCTION SET_ELEMENT : BOOL
  VAR_INPUT p : REF_TO ARRAY[0..1] OF SINT; v : SINT; END_VAR
  VAR j : DINT := 1; END_VAR
  p^[j] := v + v;
  SET_ELEMENT := TRUE;
END_FUNCTION
PROGRAM main
  VAR arr : ARRAY[0..1] OF SINT; a : SINT := 100; r : DINT; ok : BOOL; END_VAR
  ok := SET_ELEMENT(p := REF(arr), v := a);
  r := arr[1];
END_PROGRAM
";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(source, &options);
    assert_eq!(snapshot.read_as::<i32>("r"), -56);
}

/// A value stored through a dereference is truncated to the width of the type
/// the reference refers to, as one stored in that variable directly is. The
/// value is read back into a 32-bit variable of the same signedness, which
/// does not truncate.
#[spec_test(REQ_IC_codegen_021)]
#[rstest]
#[case::signed_8_bit("SINT", "100", "DINT", -56)]
#[case::unsigned_8_bit("USINT", "200", "UDINT", 144)]
#[case::signed_16_bit("INT", "20000", "DINT", -25536)]
#[case::unsigned_16_bit("UINT", "40000", "UDINT", 14464)]
fn assignment_when_narrow_variable_through_reference_overflows_then_value_wraps_at_its_width(
    #[case] ty: &str,
    #[case] a: &str,
    #[case] wide: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "
PROGRAM main
  VAR x : {ty}; a : {ty} := {a}; p : REF_TO {ty}; r : {wide}; END_VAR
  p := REF(x);
  p^ := a + a;
  r := x;
END_PROGRAM
"
    );
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(&source, &options);
    assert_eq!(snapshot.read_as::<i32>("r"), expected);
}
