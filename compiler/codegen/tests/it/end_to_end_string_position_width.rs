//! End-to-end tests for the position and length of a string function given
//! as an integer of another width than `DINT`, which the analyzer converts to
//! `DINT` (REQ-IC-analyzer-104).

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

#[test]
fn end_to_end_when_string_position_of_other_width_then_same_result_as_dint() {
    let source = "
PROGRAM main
  VAR
    s : STRING := 'abcdef';
    l : LINT := 2;
    ul : ULINT := 3;
    us : USINT := 1;
    left_r : STRING;
    right_r : STRING;
    mid_r : STRING;
    delete_r : STRING;
    insert_r : STRING;
    replace_r : STRING;
  END_VAR
  left_r := LEFT(s, l);
  right_r := RIGHT(s, ul);
  mid_r := MID(s, l, ul);
  delete_r := DELETE(s, l, us);
  insert_r := INSERT(s, 'xy', l);
  replace_r := REPLACE(s, 'Z', us, l);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("left_r"), "ab");
    assert_eq!(snapshot.read("right_r"), "def");
    assert_eq!(snapshot.read("mid_r"), "cd");
    assert_eq!(snapshot.read("delete_r"), "cdef");
    assert_eq!(snapshot.read("insert_r"), "abxycdef");
    assert_eq!(snapshot.read("replace_r"), "aZcdef");
}
