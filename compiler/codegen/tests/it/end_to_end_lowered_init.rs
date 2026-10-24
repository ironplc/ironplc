//! End-to-end tests of `init` emitted from the lowered program
//! (REQ-LOW-codegen-200): each kind of starting value, stored as the
//! declaration states it. Every program is compiled on every backend, and
//! the variables `init` leaves, then those one scan leaves, are compared by
//! name.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

/// Runs `source` for one scan, on every backend.
fn run(source: &str) -> Snapshot {
    Snapshot::run(source, &CompilerOptions::default())
}

/// A zero of each kind of scalar, of an elementary type, an enumeration and
/// a subrange, starts at zero.
#[test]
fn end_to_end_when_zero_of_each_kind_then_starts_at_zero() {
    let snapshot = run("
        TYPE
          MYINT : INT := 7;
          COLOR : (RED, GREEN, BLUE) := GREEN;
          SMALL : INT(0..5) := 3;
        END_TYPE
        PROGRAM main
          VAR
            i : INT := 0;
            m : MYINT := 0;
            c : COLOR := RED;
            r : SMALL := 0;
            f : REAL := 0.0;
            b : BOOL := FALSE;
            out : INT;
          END_VAR
          out := i + m + r;
          IF c = RED AND NOT b AND f = 0.0 THEN out := out + 1; END_IF;
        END_PROGRAM");

    assert_eq!(snapshot.read("out"), 1);
}

/// A string field of a structure and an array of strings start at their
/// initializers.
#[test]
fn end_to_end_when_string_field_and_array_of_strings_then_start_at_initializers() {
    let snapshot = run("
        TYPE P : STRUCT name : STRING[10] := 'ab'; n : INT := 3; END_STRUCT; END_TYPE
        PROGRAM main
          VAR
            p : P;
            names : ARRAY[1..3] OF STRING[5] := ['x', '', 'yz'];
            first : STRING[10];
            last : STRING[5];
            n : INT;
          END_VAR
          first := p.name;
          last := names[3];
          n := p.n + LEN(names[2]);
        END_PROGRAM");

    assert_eq!(snapshot.read("first"), "ab");
    assert_eq!(snapshot.read("last"), "yz");
    assert_eq!(snapshot.read("n"), 3);
}

/// A structure inside each element of an array of structures starts at its
/// type's default. The array's region is compared whole after `init`, as a
/// body does not read a structure inside an element yet.
#[test]
fn end_to_end_when_structure_in_array_of_structures_then_starts_at_defaults() {
    let snapshot = run("
        TYPE
          INNER : STRUCT x : INT := 4; END_STRUCT;
          OUTER : STRUCT inner : INNER; y : INT; END_STRUCT;
        END_TYPE
        PROGRAM main
          VAR
            items : ARRAY[0..1] OF OUTER;
            b : INT;
          END_VAR
          b := items[1].y + 1;
        END_PROGRAM");

    assert_eq!(snapshot.read("b"), 1);
}

/// An instance of a block that holds an instance of another starts at its
/// block's defaults, and the instance it holds cleared. The instances are
/// compared whole after `init`, as a body does not read a field of a held
/// instance yet.
#[test]
fn end_to_end_when_instance_holds_instance_then_both_start_at_defaults() {
    let snapshot = run("
        FUNCTION_BLOCK INNER
          VAR_INPUT k : INT; END_VAR
          VAR_OUTPUT q : INT; END_VAR
          q := k;
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK OUTER
          VAR_INPUT m : INT := 2; END_VAR
          VAR_OUTPUT q : INT; END_VAR
          VAR held : INNER; END_VAR
          q := m;
        END_FUNCTION_BLOCK
        PROGRAM main
          VAR o : OUTER; out : INT; END_VAR
          o();
          out := o.q;
        END_PROGRAM");

    assert_eq!(snapshot.read("out"), 2);
}

/// An instance of a user-defined block starts at its own initializer, and
/// at the block's defaults for the fields the initializer leaves out.
#[test]
fn end_to_end_when_user_block_instance_initializer_then_starts_at_it() {
    let snapshot = run("
        FUNCTION_BLOCK COUNTER
          VAR_INPUT inc : INT := 1; start : INT := 10; END_VAR
          VAR_OUTPUT q : INT; END_VAR
          q := start + inc;
        END_FUNCTION_BLOCK
        PROGRAM main
          VAR c : COUNTER := (inc := 4); out : INT; END_VAR
          c();
          out := c.q;
        END_PROGRAM");

    assert_eq!(snapshot.read("out"), 14);
}
