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

// --- String operations in a loop's condition ---

// A loop condition is evaluated once per iteration plus once more. A string
// comparison there materializes both operands into the data region through
// temp buffers, so it draws on the pool as often as the body does.
e2e_i32!(
    end_to_end_when_string_compare_in_loop_condition_then_runs_every_iteration,
    "
PROGRAM main
  VAR s : STRING[32]; i : INT; END_VAR
  s := '';
  i := 0;
  WHILE s <> 'xxxxx' DO
    s := CONCAT(s, 'x');
    i := i + 1;
  END_WHILE;
END_PROGRAM
",
    &[(1, 5)],
);

// --- Numeric-to-string conversion in a loop ---

/// The `*_TO_STRING` conversions are built-ins that write into a temp
/// buffer, a third way to allocate one, so they need loop coverage too.
#[test]
fn end_to_end_when_int_to_string_in_loop_then_runs_every_iteration() {
    let source = "
PROGRAM main
  VAR t : STRING[32]; i : INT; END_VAR
  FOR i := 1 TO 300 DO
    t := DINT_TO_STRING(42);
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_string(&bufs.data_region, string_offset(&[])), "42");
}

// --- The scan cycle is a loop too ---

/// The loops above are inside one scan. This one is the outer loop: the
/// same statement runs once per scan, hundreds of times over.
///
/// It is the same failure mode one level up. If the allocator's watermark
/// survived a scan boundary the way it used to survive a loop iteration,
/// a program would run for a while and then trap `V9009` on scan N, which
/// is far harder to attribute than failing on iteration two.
#[test]
fn end_to_end_when_string_op_runs_every_scan_then_survives_many_scans() {
    let source = "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; END_VAR
  s := 'hello';
  t := CONCAT(s, 'x');
END_PROGRAM
";
    crate::common::parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        for _ in 0..500 {
            vm.run_round(0).expect("every scan should run");
        }
    });
}

/// Both loops at once: a loop inside a scan, across many scans.
#[test]
fn end_to_end_when_loop_runs_every_scan_then_survives_many_scans() {
    let source = "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; i : INT; END_VAR
  s := 'hi';
  FOR i := 1 TO 20 DO
    t := CONCAT(s, 'x');
  END_FOR;
END_PROGRAM
";
    crate::common::parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        for _ in 0..200 {
            vm.run_round(0).expect("every scan should run");
        }
    });
}

// --- The remaining string-producing shapes, in a loop ---

/// `REPLACE`, `INSERT` and `DELETE` in one loop body. The tests above loop
/// `CONCAT`, `LEFT` and `MID`; these are the rest of the family, and three
/// in a single body also checks that consecutive operations reuse the slot
/// rather than stacking.
#[test]
fn end_to_end_when_replace_insert_delete_in_loop_then_runs_every_iteration() {
    let source = "
PROGRAM main
  VAR t : STRING[32]; i : INT; END_VAR
  t := 'abcdef';
  FOR i := 1 TO 50 DO
    t := REPLACE(t, 'X', 1, 1);
    t := INSERT(t, 'Y', 1);
    t := DELETE(t, 1, 1);
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Each iteration replaces the first character with X, inserts Y before
    // it, then deletes that Y again, so the string settles after the first.
    assert_eq!(read_string(&bufs.data_region, string_offset(&[])), "Ybcdef");
}

/// A nested string expression inside a loop. Nesting spills each inner
/// result to a data-region slot, so the whole expression still needs one
/// buffer however deep it goes -- and that has to hold on every iteration.
#[test]
fn end_to_end_when_nested_concat_in_loop_then_runs_every_iteration() {
    let source = "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; i : INT; END_VAR
  s := 'q';
  FOR i := 1 TO 50 DO
    t := CONCAT(CONCAT(s, 'a'), CONCAT(s, 'b'));
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_string(&bufs.data_region, string_offset(&[32])), "qaqb");
}

/// Reading a string array element in a loop. The test above writes one;
/// `STR_LOAD_ARRAY_ELEM` is the other array opcode that draws on the pool.
#[test]
fn end_to_end_when_string_array_element_read_in_loop_then_runs_every_iteration() {
    let source = "
PROGRAM main
  VAR names : ARRAY[1..3] OF STRING[16]; t : STRING[16]; i : INT; END_VAR
  names[1] := 'aa';
  FOR i := 1 TO 50 DO
    t := names[1];
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // `t` follows the array's three elements in the data region.
    assert_eq!(
        read_string(&bufs.data_region, string_offset(&[16, 16, 16])),
        "aa"
    );
}

/// A structure's STRING field assigned in a loop, which reaches the pool
/// through the struct field path rather than the plain variable one.
#[test]
fn end_to_end_when_struct_string_field_assigned_in_loop_then_runs_every_iteration() {
    let source = "
TYPE Rec : STRUCT name : STRING[16]; END_STRUCT; END_TYPE

PROGRAM main
  VAR r : Rec; i : INT; END_VAR
  FOR i := 1 TO 50 DO
    r.name := CONCAT('id', 'x');
  END_FOR;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_string(&bufs.data_region, string_offset(&[])), "idx");
}
