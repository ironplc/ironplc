//! Bytecode-level integration tests for temp string buffer pool sizing —
//! the `num_temp_bufs` and `max_temp_buf_bytes` header fields only. Runtime
//! behaviour is covered by `end_to_end_string_loop.rs` and
//! `end_to_end_string_operand_bounds.rs`.
//!
//! The VM releases a temp buffer when the instruction that consumes it
//! runs, so the pool has to hold the most buffers live at one time, not one
//! per string operation in the source. These tests pin that distinction:
//! repeating an operation must not grow the pool, and nesting must.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_compile;

fn num_temp_bufs(source: &str) -> u16 {
    parse_and_compile(source, &CompilerOptions::default())
        .header
        .num_temp_bufs
}

fn max_temp_buf_bytes(source: &str) -> u32 {
    parse_and_compile(source, &CompilerOptions::default())
        .header
        .max_temp_buf_bytes
}

#[test]
fn compile_when_no_string_operations_then_pool_is_empty() {
    let source = "
PROGRAM main
  VAR x : DINT; END_VAR
  x := x + 1;
END_PROGRAM
";
    assert_eq!(num_temp_bufs(source), 0);
}

#[test]
fn compile_when_one_string_assignment_then_pool_holds_one() {
    let source = "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; END_VAR
  s := 'hello';
  t := CONCAT(s, 'x');
END_PROGRAM
";
    assert_eq!(num_temp_bufs(source), 1);
}

/// Each statement spills its result into the data region before the next
/// begins, so the buffer is free again: ten statements need one buffer, not
/// ten. This is the sizing the old static-site count got wrong.
#[test]
fn compile_when_many_sequential_string_statements_then_pool_does_not_grow() {
    let source = "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; END_VAR
  s := 'a';
  t := CONCAT(s, 'b');
  t := CONCAT(t, 'c');
  t := CONCAT(t, 'd');
  t := LEFT(t, 2);
  t := RIGHT(t, 1);
  t := MID(t, 1, 1);
  t := INSERT(t, 'e', 1);
  t := DELETE(t, 1, 1);
  t := REPLACE(t, 'f', 1, 1);
END_PROGRAM
";
    assert_eq!(num_temp_bufs(source), 1);
}

/// The loop body runs 1000 times but is compiled once, and every iteration
/// hands the same slot back. A pool sized per iteration is not expressible
/// in the header, which is why the buffer has to be released on consume.
#[test]
fn compile_when_string_operation_in_loop_then_pool_matches_the_body() {
    let looped = "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; i : INT; END_VAR
  s := 'hello';
  FOR i := 1 TO 1000 DO
    t := CONCAT(s, 'x');
  END_FOR;
END_PROGRAM
";
    let unlooped = "
PROGRAM main
  VAR s : STRING[32]; t : STRING[32]; i : INT; END_VAR
  s := 'hello';
  t := CONCAT(s, 'x');
END_PROGRAM
";
    assert_eq!(num_temp_bufs(looped), num_temp_bufs(unlooped));
}

/// A called function's buffers sit on top of whatever its caller holds, so
/// the pool covers the call chain rather than one function.
#[test]
fn compile_when_string_function_called_then_pool_covers_the_call_chain() {
    let source = "
FUNCTION decorate : STRING[32]
  VAR_INPUT v : STRING[32]; END_VAR
  decorate := CONCAT(v, '!');
END_FUNCTION

PROGRAM main
  VAR t : STRING[32]; END_VAR
  t := decorate('hi');
END_PROGRAM
";
    let caller_only = "
PROGRAM main
  VAR t : STRING[32]; END_VAR
  t := CONCAT('hi', '!');
END_PROGRAM
";
    assert!(num_temp_bufs(source) > num_temp_bufs(caller_only));
}

/// An uncalled function cannot draw on the pool, so it must not be charged
/// to it — the walk starts from the scan function and follows call edges.
#[test]
fn compile_when_string_function_is_never_called_then_pool_ignores_it() {
    let with_dead_function = "
FUNCTION unused : STRING[32]
  VAR_INPUT v : STRING[32]; END_VAR
  unused := CONCAT(v, '!');
END_FUNCTION

PROGRAM main
  VAR t : STRING[32]; END_VAR
  t := CONCAT('hi', '!');
END_PROGRAM
";
    let without = "
PROGRAM main
  VAR t : STRING[32]; END_VAR
  t := CONCAT('hi', '!');
END_PROGRAM
";
    assert_eq!(
        num_temp_bufs(with_dead_function),
        num_temp_bufs(without),
        "an uncalled function should not enlarge the pool"
    );
}

/// A string variable's initial value is stored by the init function, which
/// the scan function never calls, so it is bounded on its own.
#[test]
fn compile_when_only_initial_values_use_strings_then_pool_holds_one() {
    let source = "
PROGRAM main
  VAR s : STRING[32] := 'hello'; t : STRING[32] := 'world'; END_VAR
  s := s;
END_PROGRAM
";
    assert_eq!(num_temp_bufs(source), 1);
}

/// Every string operation writes its result to a pool slot, so the slot has
/// to hold the widest operand temporary codegen materializes, not only the
/// widest declared string: a nested CONCAT of two STRING[128] is 256 units.
#[test]
fn compile_when_operand_bound_exceeds_declared_strings_then_pool_slot_holds_it() {
    let source = "
PROGRAM main
  VAR a : STRING[128]; n : INT; END_VAR
  n := LEN(CONCAT(a, a));
END_PROGRAM
";
    assert_eq!(
        max_temp_buf_bytes(source),
        ironplc_container::STRING_HEADER_BYTES as u32 + 256
    );
}

/// A literal is loaded into a pool slot before it reaches its temporary, so
/// its own length has to size the slot when nothing declared is as long.
#[test]
fn compile_when_literal_longer_than_declared_strings_then_pool_slot_holds_it() {
    let literal = "a".repeat(300);
    let source = format!(
        "
PROGRAM main
  VAR s : STRING[10]; n : INT; END_VAR
  n := LEN('{literal}');
END_PROGRAM
"
    );
    assert_eq!(
        max_temp_buf_bytes(&source),
        ironplc_container::STRING_HEADER_BYTES as u32 + 300
    );
}
