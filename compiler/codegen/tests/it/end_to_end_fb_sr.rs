//! End-to-end tests for function block invocation (SR set-reset bistable).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! an SR function block instance, compile to bytecode, and execute on the VM.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, FbStep, FbStep::*};

const SR_PROGRAM: &str = "
PROGRAM main
  VAR
    latch : SR;
    set_in : BOOL;
    reset_in : BOOL;
    result : BOOL;
  END_VAR
  latch(S1 := set_in, R := reset_in, Q1 => result);
END_PROGRAM
";

#[rstest]
// Both inputs FALSE: Q1 stays FALSE.
#[case::both_false(&[Run(0), Expect(3, 0)])]
// S1 alone latches Q1 TRUE, and it stays TRUE after S1 is removed.
#[case::set_latches(&[
    Write(1, 1), Run(0), Expect(3, 1),
    Write(1, 0), Run(1), Expect(3, 1),
])]
// Reset after set clears Q1.
#[case::reset_after_set(&[
    Write(1, 1), Run(0), Expect(3, 1),
    Write(1, 0), Write(2, 1), Run(1), Expect(3, 0),
])]
// Both TRUE: set dominates for SR, so Q1 is TRUE.
#[case::both_true_set_dominates(&[
    Write(1, 1), Write(2, 1), Run(0), Expect(3, 1),
])]
fn end_to_end_fb_sr(#[case] steps: &[FbStep]) {
    drive_fb(SR_PROGRAM, &CompilerOptions::default(), steps);
}
