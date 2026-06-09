//! End-to-end codegen tests for `header.max_call_depth` computation.
//!
//! Each test compiles a small IEC 61131-3 program and asserts the
//! resulting container header carries the call depth the codegen
//! analysis is supposed to produce.

use ironplc_codegen::CodegenOptions;
use ironplc_parser::options::CompilerOptions;

use crate::common::try_parse_and_compile;

fn compile_for_depth(source: &str) -> u16 {
    let container =
        try_parse_and_compile(source, &CompilerOptions::default()).expect("source should compile");
    container.header.max_call_depth
}

#[test]
fn compile_when_program_has_no_calls_then_max_call_depth_is_one() {
    // SCAN does arithmetic only — no CALL, no FB_CALL. Only the
    // entry frame is on the stack at any point, so depth = 1.
    let source = "
PROGRAM main
  VAR x : INT; END_VAR
  x := 1 + 2;
END_PROGRAM
";
    assert_eq!(compile_for_depth(source), 1);
}

#[test]
fn compile_when_program_calls_one_function_then_max_call_depth_is_two() {
    // SCAN -> ADD_ONE. Two frames: SCAN, then ADD_ONE.
    let source = "
FUNCTION ADD_ONE : INT
  VAR_INPUT x : INT; END_VAR
  ADD_ONE := x + 1;
END_FUNCTION

PROGRAM main
  VAR y : INT; END_VAR
  y := ADD_ONE(x := 5);
END_PROGRAM
";
    assert_eq!(compile_for_depth(source), 2);
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
fn compile_when_diamond_call_graph_then_max_call_depth_takes_longest() {
    // SCAN -> {SHORT, LONG_A}
    // SHORT returns directly                 (path length 2)
    // LONG_A -> LONG_B -> LEAF               (path length 4)
    // Verifies the longest path wins, not the average / first / etc.
    let source = "
FUNCTION SHORT : INT
  VAR_INPUT n : INT; END_VAR
  SHORT := n;
END_FUNCTION

FUNCTION LEAF : INT
  VAR_INPUT n : INT; END_VAR
  LEAF := n;
END_FUNCTION

FUNCTION LONG_B : INT
  VAR_INPUT n : INT; END_VAR
  LONG_B := LEAF(n := n);
END_FUNCTION

FUNCTION LONG_A : INT
  VAR_INPUT n : INT; END_VAR
  LONG_A := LONG_B(n := n);
END_FUNCTION

PROGRAM main
  VAR a : INT; b : INT; END_VAR
  a := SHORT(n := 1);
  b := LONG_A(n := 2);
END_PROGRAM
";
    assert_eq!(compile_for_depth(source), 4);
}

#[test]
fn compile_when_program_calls_user_fb_then_fb_body_counted() {
    // SCAN -> CTR.body. Two frames.
    let source = "
FUNCTION_BLOCK CTR
  VAR n : INT; END_VAR
  n := n + 1;
END_FUNCTION_BLOCK

PROGRAM main
  VAR c : CTR; END_VAR
  c();
END_PROGRAM
";
    assert_eq!(compile_for_depth(source), 2);
}

#[test]
fn compile_when_user_fb_body_calls_user_function_then_both_counted() {
    // SCAN -> CTR.body -> ADD_ONE. Three frames at the deepest point.
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

#[test]
fn compile_when_codegen_options_default_then_header_has_max_call_depth_set() {
    // Smoke-test the wiring: regardless of options, every compiled
    // program ends up with a populated `max_call_depth` (>= 1).
    let _ = CodegenOptions::default();
    let source = "
PROGRAM main
  VAR x : INT; END_VAR
  x := 0;
END_PROGRAM
";
    assert!(compile_for_depth(source) >= 1);
}
