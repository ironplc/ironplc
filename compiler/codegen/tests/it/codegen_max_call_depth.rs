//! End-to-end codegen tests for `header.max_call_depth` computation.
//!
//! Each test compiles a small IEC 61131-3 program and asserts the
//! resulting container header carries the call depth the codegen
//! analysis is supposed to produce.
//!
//! The depth *algorithm* (base case, linear chains of any length,
//! diamonds/longest-path, cycles) is exhaustively unit-tested in
//! `codegen/src/call_graph.rs`. These integration tests only cover the
//! source -> compile -> header wiring, so we keep two representatives:
//! one that drives function `CALL` edges through a multi-level chain, and
//! one that drives `FB_CALL` and `CALL` edges together.

use ironplc_parser::options::CompilerOptions;

use crate::common::try_parse_and_compile;

fn compile_for_depth(source: &str) -> u16 {
    let container = try_parse_and_compile(source, &CompilerOptions::default()).unwrap();
    container.header.max_call_depth
}

#[test]
fn compile_when_call_chain_three_deep_then_max_call_depth_is_four() {
    // SCAN -> A -> B -> C. Four frames at the deepest point.
    let source = "
FUNCTION C : INT
  VAR_INPUT n : INT; END_VAR
  C := n;
END_FUNCTION

FUNCTION B : INT
  VAR_INPUT n : INT; END_VAR
  B := C(n := n);
END_FUNCTION

FUNCTION A : INT
  VAR_INPUT n : INT; END_VAR
  A := B(n := n);
END_FUNCTION

PROGRAM main
  VAR y : INT; END_VAR
  y := A(n := 7);
END_PROGRAM
";
    assert_eq!(compile_for_depth(source), 4);
}

#[test]
fn compile_when_user_fb_body_calls_user_function_then_both_counted() {
    // SCAN -> CTR.body -> ADD_ONE. Three frames at the deepest point.
    // Exercises both FB_CALL (main -> CTR) and CALL (CTR -> ADD_ONE) edges.
    let source = "
FUNCTION ADD_ONE : INT
  VAR_INPUT x : INT; END_VAR
  ADD_ONE := x + 1;
END_FUNCTION

FUNCTION_BLOCK CTR
  VAR n : INT; END_VAR
  n := ADD_ONE(x := n);
END_FUNCTION_BLOCK

PROGRAM main
  VAR c : CTR; END_VAR
  c();
END_PROGRAM
";
    assert_eq!(compile_for_depth(source), 3);
}
