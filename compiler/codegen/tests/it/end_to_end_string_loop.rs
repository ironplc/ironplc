//! End-to-end integration tests for string operations inside loops.
//!
//! A string operation allocates a temporary buffer from the VM's pool, and
//! the pool is sized statically. These tests pin the property that makes
//! that sizing sound: the buffer is released when the instruction that
//! consumes it runs, so running the same operation `n` times draws one
//! buffer, not `n`. Before that, a loop over any string-producing function
//! trapped `V9009` on its second iteration (issue #1590).
//!
//! The iteration counts here are far above any plausible static site count,
//! so a regression to per-iteration allocation cannot pass by being sized
//! generously.

use ironplc_parser::options::CompilerOptions;

use crate::common::{parse_and_run, read_string, string_offset};

// --- The reported reproducer ---

// i is variable slot 2, n is slot 3 (s and t live in the data region).
e2e_i32!(
    end_to_end_when_concat_in_for_loop_then_runs_every_iteration,
    "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; i : INT; n : INT; END_VAR
  s := 'hello';
  FOR i := 1 TO 2 DO
    t := CONCAT(s, 'x');
  END_FOR;
  n := LEN(t);
END_PROGRAM
",
    &[(3, 6)],
);

e2e_i32!(
    end_to_end_when_concat_in_long_for_loop_then_runs_every_iteration,
    "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; i : INT; n : INT; END_VAR
  s := 'hello';
  FOR i := 1 TO 1000 DO
    t := CONCAT(s, 'x');
  END_FOR;
  n := LEN(t);
END_PROGRAM
",
    &[(3, 6)],
);

// --- The other loop forms ---

e2e_i32!(
    end_to_end_when_string_op_in_while_loop_then_runs_every_iteration,
    "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; i : INT; n : INT; END_VAR
  s := 'hello';
  i := 0;
  WHILE i < 500 DO
    t := LEFT(s, 3);
    i := i + 1;
  END_WHILE;
  n := LEN(t);
END_PROGRAM
",
    &[(3, 3)],
);

e2e_i32!(
    end_to_end_when_string_op_in_repeat_loop_then_runs_every_iteration,
    "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; i : INT; n : INT; END_VAR
  s := 'hello';
  i := 0;
  REPEAT
    t := MID(s, 2, 2);
    i := i + 1;
  UNTIL i >= 500
  END_REPEAT;
  n := LEN(t);
END_PROGRAM
",
    &[(3, 2)],
);

// --- Accumulating into the same variable across iterations ---

/// `t := CONCAT(t, '.')` reads and writes one variable each iteration, so
/// the result also proves the buffer is copied out before its slot is
/// reused rather than aliased.
#[test]
fn end_to_end_when_concat_accumulates_in_loop_then_builds_the_whole_string() {
    let source = "
PROGRAM main
  VAR t : STRING[32]; i : INT; END_VAR
  t := '';
  FOR i := 1 TO 8 DO
    t := CONCAT(t, '.');
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(
        read_string(&bufs.data_region, string_offset(&[])),
        "........"
    );
}

/// A nested loop drives the body 20 times through two loop levels, with a
/// second string operation between them.
#[test]
fn end_to_end_when_string_ops_nested_in_two_loops_then_builds_the_whole_string() {
    let source = "
PROGRAM main
  VAR t : STRING[32]; u : STRING[32]; i : INT; j : INT; END_VAR
  t := '';
  FOR i := 1 TO 4 DO
    u := LEFT('abcd', i);
    FOR j := 1 TO 5 DO
      t := CONCAT(u, 'z');
    END_FOR;
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_string(&bufs.data_region, string_offset(&[])), "abcdz");
}

// --- A string-returning function called in a loop ---

/// The callee allocates a buffer for its return value that outlives its own
/// frame, so this exercises the caller-plus-callee half of the pool bound.
#[test]
fn end_to_end_when_string_function_called_in_loop_then_runs_every_iteration() {
    let source = "
FUNCTION decorate : STRING[32]
  VAR_INPUT v : STRING[32]; END_VAR
  decorate := CONCAT(v, '!');
END_FUNCTION

PROGRAM main
  VAR t : STRING[32]; i : INT; END_VAR
  FOR i := 1 TO 200 DO
    t := decorate('hi');
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_string(&bufs.data_region, string_offset(&[])), "hi!");
}

// --- A string array element written in a loop ---

/// `STR_STORE_ARRAY_ELEM` is the second opcode that consumes a `buf_idx`,
/// so it needs its own loop coverage.
#[test]
fn end_to_end_when_string_array_element_assigned_in_loop_then_runs_every_iteration() {
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..3] OF STRING[16];
    t : STRING[16];
    i : INT;
  END_VAR
  FOR i := 1 TO 3 DO
    names[i] := CONCAT('id', 'x');
  END_FOR;
  t := names[2];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // The array occupies the data region first; `t` follows its 3 elements.
    assert_eq!(
        read_string(&bufs.data_region, string_offset(&[16, 16, 16])),
        "idx"
    );
}
