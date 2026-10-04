//! End-to-end tests for F_TRIG (falling edge detector) function block.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, expect, run, write, FbStep};

const PROGRAM: &str = "
PROGRAM main
  VAR
    edge : F_TRIG;
    clk : BOOL;
    result : BOOL;
  END_VAR
  edge(CLK := clk, Q => result);
END_PROGRAM
";

#[rstest]
// CLK stays FALSE across the first scan: no falling edge, Q FALSE.
#[case::clk_false(&[run(0), expect("result", 0)])]
// Falling edge on CLK: Q pulses TRUE for one scan.
#[case::falling_edge(&[
    write("clk", 1), run(0), expect("result", 0),
    write("clk", 0), run(1), expect("result", 1),
])]
// CLK held FALSE after the edge: Q returns to FALSE on the next scan.
#[case::clk_stays_false(&[
    write("clk", 1), run(0),
    write("clk", 0), run(1), expect("result", 1),
    run(2), expect("result", 0),
])]
// A second falling edge makes Q pulse TRUE again.
#[case::second_falling_edge(&[
    write("clk", 1), run(0),
    write("clk", 0), run(1), expect("result", 1),
    write("clk", 1), run(2), expect("result", 0),
    write("clk", 0), run(3), expect("result", 1),
])]
fn end_to_end_fb_f_trig(#[case] steps: &[FbStep]) {
    drive_fb(PROGRAM, &CompilerOptions::default(), steps);
}
