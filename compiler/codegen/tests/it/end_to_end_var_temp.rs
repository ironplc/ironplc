//! End-to-end integration tests for VAR_TEMP declarations.

use ironplc_parser::options::CompilerOptions;

use crate::common::run_scans;

e2e_i32!(
    end_to_end_when_function_with_var_temp_then_correct,
    "
FUNCTION add_doubled : DINT
  VAR_INPUT
    a : DINT;
    b : DINT;
  END_VAR
  VAR_TEMP
    temp : DINT;
  END_VAR
  temp := a + b;
  add_doubled := temp * 2;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := add_doubled(3, 4);
END_PROGRAM
",
    &[("result", 14)],
);

e2e_i32!(
    end_to_end_when_function_with_multiple_var_temp_then_correct,
    "
FUNCTION compute : DINT
  VAR_INPUT
    x : DINT;
  END_VAR
  VAR_TEMP
    a : DINT;
    b : DINT;
  END_VAR
  a := x + 1;
  b := a * 3;
  compute := b;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := compute(4);
END_PROGRAM
",
    &[("result", 15)],
);

/// Runs `source` for two scans and returns what `names` read after each.
fn two_scans(source: &str, options: &CompilerOptions, names: &[&str]) -> [Vec<i64>; 2] {
    let mut after = [Vec::new(), Vec::new()];
    run_scans(source, options, |session| {
        for (time_us, values) in [0, 1000].into_iter().zip(after.iter_mut()) {
            session.scan(time_us).unwrap();
            *values = names.iter().map(|name| session.read_as(name)).collect();
        }
    });
    after
}

/// Each scan of a program reads its `VAR_TEMP` variables before it writes
/// them, so each scan reading what the first did shows every one started
/// again from its initial value: an initialized and an uninitialized scalar,
/// a subrange, a string, a structure, a partially initialized array, a
/// two-dimensional array and an array field of a structure.
const PROGRAM_VAR_TEMP: &str = "
TYPE Small : INT(3..9); END_TYPE
TYPE Pair : STRUCT a : DINT; b : DINT := 7; END_STRUCT; END_TYPE
TYPE Holder : STRUCT items : ARRAY[1..2] OF DINT; END_STRUCT; END_TYPE

PROGRAM main
  VAR
    initialized : DINT; zeroed : DINT; lower : DINT; length : DINT;
    field_a : DINT; field_b : DINT;
    first : DINT; second : DINT; third : DINT;
    corner : DINT; nested : DINT;
  END_VAR
  VAR_TEMP
    t : DINT := 5;
    u : DINT;
    r : Small;
    s : STRING[10] := 'ab';
    p : Pair;
    arr : ARRAY[1..3] OF DINT := [1];
    m : ARRAY[1..2, 1..2] OF DINT;
    h : Holder;
  END_VAR
  t := t + 1; initialized := t;
  u := u + 1; zeroed := u;
  lower := r; r := 9;
  s := CONCAT(s, 'x'); length := LEN(s);
  p.a := p.a + 1; p.b := p.b + 1; field_a := p.a; field_b := p.b;
  arr[1] := arr[1] + 10; arr[2] := arr[2] + 10;
  first := arr[1]; second := arr[2]; third := arr[3]; arr[3] := 99;
  m[2, 2] := m[2, 2] + 1; corner := m[2, 2];
  h.items[2] := h.items[2] + 1; nested := h.items[2];
END_PROGRAM
";

#[test]
fn end_to_end_when_program_var_temp_then_starts_again_each_scan() {
    let names = [
        "initialized",
        "zeroed",
        "lower",
        "length",
        "field_a",
        "field_b",
        "first",
        "second",
        "third",
        "corner",
        "nested",
    ];
    let expected = vec![6, 1, 3, 3, 1, 8, 11, 10, 0, 1, 1];
    assert_eq!(
        two_scans(PROGRAM_VAR_TEMP, &CompilerOptions::default(), &names),
        [expected.clone(), expected]
    );
}

/// The elements of a program's `VAR_TEMP` array of references start each
/// scan as NULL.
const PROGRAM_VAR_TEMP_REFERENCES: &str = "
PROGRAM main
  VAR was_null : BOOL; target : DINT; END_VAR
  VAR_TEMP refs : ARRAY[1..2] OF REF_TO DINT; END_VAR
  was_null := refs[1] = NULL;
  refs[1] := REF(target);
END_PROGRAM
";

#[test]
fn end_to_end_when_program_var_temp_reference_array_then_null_each_scan() {
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    assert_eq!(
        two_scans(PROGRAM_VAR_TEMP_REFERENCES, &options, &["was_null"]),
        [vec![1], vec![1]]
    );
}
