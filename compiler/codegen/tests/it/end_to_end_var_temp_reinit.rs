//! End-to-end tests for `VAR_TEMP` (#1857): a temporary variable starts from
//! its initial value on every execution of its POU, in a PROGRAM (every
//! scan) and in a FUNCTION (every call).

use ironplc_container::VarIndex;
use ironplc_parser::options::{CompilerOptions, Dialect};

use crate::common::parse_and_run_rounds;

#[test]
fn end_to_end_when_program_var_temp_then_reinitialized_every_scan() {
    // q accumulates t, and t is 5 + 1 on every scan: 6, then 12. A t kept
    // across scans would give 6, then 13.
    let source = "
PROGRAM main
  VAR q : INT; END_VAR
  VAR_TEMP t : INT := 5; END_VAR
  t := t + 1;
  q := q + t;
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 6);
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 12);
    });
}

#[test]
fn end_to_end_when_program_var_temp_first_section_then_accepted() {
    let source = "
PROGRAM main
  VAR_TEMP t : DINT; END_VAR
  VAR q : DINT; END_VAR
  t := t + 7;
  q := q + t;
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 14);
    });
}

#[test]
fn end_to_end_when_function_var_temp_then_reinitialized_every_call() {
    let source = "
FUNCTION f : INT
  VAR_TEMP t : INT := 1; END_VAR
  t := t + 1;
  f := t;
END_FUNCTION
PROGRAM main
  VAR a : INT; b : INT; END_VAR
  a := f();
  b := f();
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 2);
        assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 2);
    });
}

#[test]
fn end_to_end_when_program_var_temp_structure_then_reinitialized_every_scan() {
    let source = "
TYPE P : STRUCT x : INT; END_STRUCT; END_TYPE
PROGRAM main
  VAR q : INT; END_VAR
  VAR_TEMP p : P := (x := 5); END_VAR
  p.x := p.x + 1;
  q := q + p.x;
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 12);
    });
}

/// Runs `source` for two scans and returns the values of the variables at
/// `indices` after the second one.
fn after_two_scans(source: &str, options: &CompilerOptions, indices: &[u16]) -> Vec<i32> {
    let mut values = Vec::new();
    parse_and_run_rounds(source, options, |vm| {
        vm.run_round(0).unwrap();
        vm.run_round(0).unwrap();
        for index in indices {
            values.push(vm.read_variable(VarIndex::new(*index)).unwrap());
        }
    });
    values
}

#[test]
fn end_to_end_when_program_var_temp_array_partially_initialized_then_every_element_reset_every_scan(
) {
    // Each scan adds the elements to q1..q3, then overwrites all of them.
    // a[2] and a[3] are past the end of the initializer, so they restart
    // from INT's default, 0, not from the previous scan's 20 and 30.
    let source = "
PROGRAM main
  VAR q1 : INT; q2 : INT; q3 : INT; END_VAR
  VAR_TEMP a : ARRAY[1..3] OF INT := [1]; END_VAR
  q1 := q1 + a[1];
  q2 := q2 + a[2];
  q3 := q3 + a[3];
  a[1] := 10;
  a[2] := 20;
  a[3] := 30;
END_PROGRAM
";
    assert_eq!(
        after_two_scans(source, &CompilerOptions::default(), &[0, 1, 2]),
        vec![2, 0, 0]
    );
}

#[test]
fn end_to_end_when_program_var_temp_array_without_initializer_then_every_element_reset_every_scan()
{
    let source = "
PROGRAM main
  VAR q1 : DINT; q2 : DINT; END_VAR
  VAR_TEMP a : ARRAY[0..1, 0..1] OF DINT; END_VAR
  q1 := q1 + a[0, 0];
  q2 := q2 + a[1, 1];
  a[0, 0] := 5;
  a[1, 1] := 7;
END_PROGRAM
";
    assert_eq!(
        after_two_scans(source, &CompilerOptions::default(), &[0, 1]),
        vec![0, 0]
    );
}

#[test]
fn end_to_end_when_program_var_temp_reference_array_then_elements_null_every_scan() {
    // A reference's default is NULL, which is not a zero slot.
    let source = "
PROGRAM main
  VAR x : INT; n1 : INT; n2 : INT; END_VAR
  VAR_TEMP r : ARRAY[1..2] OF REF_TO INT; END_VAR
  IF r[1] = NULL THEN n1 := n1 + 1; END_IF;
  IF r[2] = NULL THEN n2 := n2 + 1; END_IF;
  r[2] := REF(x);
END_PROGRAM
";
    assert_eq!(
        after_two_scans(
            source,
            &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
            &[1, 2]
        ),
        vec![2, 2]
    );
}

#[test]
fn end_to_end_when_program_var_temp_string_array_then_every_element_reset_every_scan() {
    let source = "
PROGRAM main
  VAR n1 : INT; n2 : INT; END_VAR
  VAR_TEMP s : ARRAY[1..2] OF STRING[8] := ['ab']; END_VAR
  n1 := n1 + LEN(s[1]);
  n2 := n2 + LEN(s[2]);
  s[1] := 'wxyz';
  s[2] := 'xyz';
END_PROGRAM
";
    assert_eq!(
        after_two_scans(source, &CompilerOptions::default(), &[0, 1]),
        vec![4, 0]
    );
}

#[test]
fn end_to_end_when_program_var_temp_structure_array_field_then_every_element_reset_every_scan() {
    // The array field's initializer covers a[1] only: a[2] restarts from 0.
    let source = "
TYPE S : STRUCT n : INT; a : ARRAY[1..3] OF INT; END_STRUCT; END_TYPE
PROGRAM main
  VAR q1 : INT; q2 : INT; END_VAR
  VAR_TEMP p : S := (a := [4]); END_VAR
  q1 := q1 + p.a[1];
  q2 := q2 + p.a[2];
  p.a[1] := 10;
  p.a[2] := 20;
END_PROGRAM
";
    assert_eq!(
        after_two_scans(source, &CompilerOptions::default(), &[0, 1]),
        vec![8, 0]
    );
}

#[test]
fn end_to_end_when_program_var_temp_structure_subrange_array_field_then_reset_to_lower_bound() {
    // A subrange's default is its lower bound, not zero.
    let source = "
TYPE Rng : INT(5..10); END_TYPE
TYPE S : STRUCT a : ARRAY[1..2] OF Rng; END_STRUCT; END_TYPE
PROGRAM main
  VAR q : INT; END_VAR
  VAR_TEMP p : S; END_VAR
  q := q + p.a[2];
  p.a[2] := 9;
END_PROGRAM
";
    assert_eq!(
        after_two_scans(source, &CompilerOptions::default(), &[0]),
        vec![10]
    );
}

#[test]
fn end_to_end_when_program_var_temp_array_of_structures_then_every_element_reset_every_scan() {
    let source = "
TYPE P : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
TYPE S : STRUCT items : ARRAY[1..2] OF P; END_STRUCT; END_TYPE
PROGRAM main
  VAR q1 : INT; q2 : INT; END_VAR
  VAR_TEMP arr : ARRAY[1..2] OF P; s : S; END_VAR
  q1 := q1 + arr[2].y;
  q2 := q2 + s.items[2].x;
  arr[2].y := 9;
  s.items[2].x := 7;
END_PROGRAM
";
    assert_eq!(
        after_two_scans(source, &CompilerOptions::default(), &[0, 1]),
        vec![0, 0]
    );
}

#[test]
fn end_to_end_when_program_var_temp_subrange_then_reset_to_lower_bound_every_scan() {
    let source = "
TYPE Rng : INT(5..10); END_TYPE
PROGRAM main
  VAR q : INT; END_VAR
  VAR_TEMP r : Rng; END_VAR
  q := q + r;
  r := 9;
END_PROGRAM
";
    assert_eq!(
        after_two_scans(source, &CompilerOptions::default(), &[0]),
        vec![10]
    );
}
