//! End-to-end integration tests for function forms of operators.
//!
//! One smoke test per category (arithmetic, comparison) to verify the full
//! pipeline works. Detailed opcode testing is in compile_func_forms.rs.
//!
//! Note: Boolean function forms (AND, OR, XOR, NOT) and MOD are not tested
//! because the parser treats these names as operator keywords.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_add_function_then_returns_sum() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := ADD(x, 32);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_le_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := LE(5, 10);
  false_result := LE(10, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1); // TRUE
    assert_eq!(bufs.vars[1].as_i32(), 0); // FALSE
}
