//! End-to-end tests for function block invocation (RS reset-set bistable).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! an RS function block instance, compile to bytecode, and execute on the VM.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, expect, run, write, FbStep};

const RS_PROGRAM: &str = "
PROGRAM main
  VAR
    latch : RS;
    set_in : BOOL;
    reset_in : BOOL;
    result : BOOL;
  END_VAR
  latch(S := set_in, R1 := reset_in, Q1 => result);
END_PROGRAM
";

#[rstest]
// Both inputs FALSE: Q1 stays FALSE.
#[case::both_false(&[run(0), expect("result", 0)])]
// S alone latches Q1 TRUE, and it stays TRUE after S is removed.
#[case::set_latches(&[
    write("set_in", 1), run(0), expect("result", 1),
    write("set_in", 0), run(1), expect("result", 1),
])]
// Reset after set clears Q1.
#[case::reset_after_set(&[
    write("set_in", 1), run(0), expect("result", 1),
    write("set_in", 0), write("reset_in", 1), run(1), expect("result", 0),
])]
// Both TRUE: reset dominates for RS, so Q1 is FALSE.
#[case::both_true_reset_dominates(&[
    write("set_in", 1), write("reset_in", 1), run(0), expect("result", 0),
])]
fn end_to_end_fb_rs(#[case] steps: &[FbStep]) {
    drive_fb(RS_PROGRAM, &CompilerOptions::default(), steps);
}
