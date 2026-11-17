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

/// The function block of #1969: each call reads its `VAR_TEMP` before it
/// writes it, and adds it to an output the instance keeps, so `q` is 6
/// after the first scan and 12 after the second only when `t` starts again
/// from 5 on every call.
const BLOCK_VAR_TEMP: &str = "
FUNCTION_BLOCK Acc
  VAR_OUTPUT q : INT; END_VAR
  VAR_TEMP t : INT := 5; END_VAR
  t := t + 1;
  q := q + t;
END_FUNCTION_BLOCK

PROGRAM main
  VAR o : Acc; q : INT; END_VAR
  o();
  q := o.q;
END_PROGRAM
";

#[test]
fn end_to_end_when_function_block_var_temp_then_starts_again_each_call() {
    assert_eq!(
        two_scans(BLOCK_VAR_TEMP, &CompilerOptions::default(), &["q"]),
        [vec![6], vec![12]]
    );
}

/// Each call of the block reads each `VAR_TEMP` before writing it, so every
/// call reading what the first did shows each started again from its
/// starting value: an initialized and an uninitialized scalar, a subrange,
/// a string and a reference. Two instances of the block, each called twice
/// a scan, share the block's `VAR_TEMP` slots, and neither sees what the
/// other's call left.
const BLOCK_VAR_TEMPS: &str = "
TYPE Small : INT(3..9); END_TYPE

FUNCTION_BLOCK Probe
  VAR_OUTPUT
    initialized : DINT; zeroed : DINT; lower : DINT; length : DINT;
    was_null : BOOL;
  END_VAR
  VAR kept : DINT; END_VAR
  VAR_TEMP
    t : DINT := 5;
    u : DINT;
    r : Small;
    s : STRING[10] := 'ab';
    p : REF_TO DINT;
  END_VAR
  t := t + 1; initialized := t;
  u := u + 1; zeroed := u;
  lower := r; r := 9;
  s := CONCAT(s, 'x'); length := LEN(s);
  was_null := p = NULL; p := REF(kept);
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    a : Probe; b : Probe;
    initialized : DINT; zeroed : DINT; lower : DINT; length : DINT;
    was_null : BOOL;
  END_VAR
  a(); b(); a(); b();
  initialized := a.initialized + b.initialized;
  zeroed := a.zeroed + b.zeroed;
  lower := a.lower + b.lower;
  length := a.length + b.length;
  was_null := a.was_null AND b.was_null;
END_PROGRAM
";

#[test]
fn end_to_end_when_function_block_var_temps_then_each_starts_again_each_call() {
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let names = ["initialized", "zeroed", "lower", "length", "was_null"];
    let expected = vec![12, 2, 6, 6, 1];
    assert_eq!(
        two_scans(BLOCK_VAR_TEMPS, &options, &names),
        [expected.clone(), expected]
    );
}

/// A function block's methods take their slots after the block's
/// `VAR_TEMP`, in the frame the method's call spans: a block with both
/// compiles, its body's `VAR_TEMP` starts again on every call of the body,
/// and a call of a method between two calls of the body changes only the
/// fields.
const BLOCK_VAR_TEMP_AND_METHOD: &str = "
FUNCTION_BLOCK Counter
  VAR_OUTPUT q : DINT; END_VAR
  VAR total : DINT; END_VAR
  VAR_TEMP t : DINT := 10; END_VAR
  t := t + 1;
  total := total + t;
  q := total;
  METHOD Bump : DINT
    VAR_INPUT amount : DINT; END_VAR
    VAR_TEMP scaled : DINT; END_VAR
    scaled := scaled + amount * 100;
    total := total + scaled;
    Bump := total;
  END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
  VAR c : Counter; q : DINT; bumped : DINT; END_VAR
  c();
  bumped := c.Bump(amount := 1);
  c();
  q := c.q;
END_PROGRAM
";

#[test]
fn end_to_end_when_function_block_var_temp_and_method_then_each_has_own_slots() {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_eq!(
        two_scans(BLOCK_VAR_TEMP_AND_METHOD, &options, &["bumped", "q"]),
        [vec![111, 122], vec![233, 244]]
    );
}
