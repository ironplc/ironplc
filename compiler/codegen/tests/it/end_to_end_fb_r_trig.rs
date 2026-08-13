//! End-to-end tests for R_TRIG (rising edge detector) function block.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, FbStep, FbStep::*};

const PROGRAM: &str = "
PROGRAM main
  VAR
    edge : R_TRIG;
    clk : BOOL;
    result : BOOL;
  END_VAR
  edge(CLK := clk, Q => result);
END_PROGRAM
";

#[rstest]
// CLK stays FALSE across the first scan: no rising edge, Q FALSE.
#[case::clk_false(&[Run(0), Expect(2, 0)])]
// Rising edge on CLK: Q pulses TRUE for one scan.
#[case::rising_edge(&[Run(0), Write(1, 1), Run(1), Expect(2, 1)])]
// CLK held TRUE after the edge: Q returns to FALSE on the next scan.
#[case::clk_stays_true(&[Write(1, 1), Run(0), Expect(2, 1), Run(1), Expect(2, 0)])]
// A second rising edge (after CLK falls) makes Q pulse TRUE again.
#[case::second_rising_edge(&[
    Write(1, 1), Run(0), Expect(2, 1),
    Write(1, 0), Run(1), Expect(2, 0),
    Write(1, 1), Run(2), Expect(2, 1),
])]
fn end_to_end_fb_r_trig(#[case] steps: &[FbStep]) {
    drive_fb(PROGRAM, &CompilerOptions::default(), steps);
}
