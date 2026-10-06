//! End-to-end integration tests for ARRAY OF STRING[N] support.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

#[test]
fn array_of_string_when_assign_then_stores_value() {
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..3] OF STRING[10];
    r1 : STRING[10];
    r2 : STRING[10];
    r3 : STRING[10];
  END_VAR
  names[1] := 'hello';
  names[2] := 'world';
  r1 := names[1];
  r2 := names[2];
  r3 := names[3];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("r1"), "hello");
    assert_eq!(snapshot.read("r2"), "world");
    // Element 3 was not assigned, so it is empty.
    assert_eq!(snapshot.read("r3"), "");
}

#[test]
fn array_of_string_when_read_back_then_value_matches() {
    // Assign a string, then copy it to a scalar STRING variable.
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF STRING[10];
    result : STRING[10];
  END_VAR
  arr[2] := 'test';
  result := arr[2];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("result"), "test");
}

#[test]
fn array_of_string_when_initial_values_then_populated() {
    let source = "
PROGRAM main
  VAR
    days : ARRAY[1..3] OF STRING[10] := ['Mon', 'Tue', 'Wed'];
    r1 : STRING[10];
    r2 : STRING[10];
    r3 : STRING[10];
  END_VAR
  r1 := days[1];
  r2 := days[2];
  r3 := days[3];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("r1"), "Mon");
    assert_eq!(snapshot.read("r2"), "Tue");
    assert_eq!(snapshot.read("r3"), "Wed");
}

#[test]
fn array_of_string_when_multidim_then_correct_indexing() {
    let source = "
PROGRAM main
  VAR
    grid : ARRAY[1..2, 1..2] OF STRING[5];
    r11 : STRING[5];
    r12 : STRING[5];
    r21 : STRING[5];
    r22 : STRING[5];
  END_VAR
  grid[1, 1] := 'a';
  grid[1, 2] := 'bb';
  grid[2, 1] := 'ccc';
  grid[2, 2] := 'dddd';
  r11 := grid[1, 1];
  r12 := grid[1, 2];
  r21 := grid[2, 1];
  r22 := grid[2, 2];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("r11"), "a");
    assert_eq!(snapshot.read("r12"), "bb");
    assert_eq!(snapshot.read("r21"), "ccc");
    assert_eq!(snapshot.read("r22"), "dddd");
}

#[test]
fn array_of_string_when_truncated_then_respects_max_length() {
    // The copy target is longer than the element, so only the element's own
    // length can cut the value.
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..2] OF STRING[3];
    r : STRING[10];
  END_VAR
  arr[1] := 'abcdefgh';
  r := arr[1];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("r"), "abc");
}

#[test]
fn array_of_string_when_default_length_then_uses_254() {
    let source = format!(
        "
PROGRAM main
  VAR
    arr : ARRAY[1..2] OF STRING;
    r1 : STRING[400];
    r2 : STRING[400];
  END_VAR
  arr[1] := '{long}';
  arr[2] := 'hello';
  r1 := arr[1];
  r2 := arr[2];
END_PROGRAM
",
        long = "a".repeat(300),
    );
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());

    assert_eq!(snapshot.read("r1"), "a".repeat(254));
    assert_eq!(snapshot.read("r2"), "hello");
}
