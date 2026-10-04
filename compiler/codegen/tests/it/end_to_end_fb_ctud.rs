//! End-to-end tests for function block invocation (CTUD count up/down counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTUD function block instance, compile to bytecode, and execute on the VM.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, expect, pulse, run, write, FbStep};

const CTUD_PROGRAM: &str = "
PROGRAM main
  VAR
    counter : CTUD;
    cu_in : BOOL;
    cd_in : BOOL;
    reset : BOOL;
    load : BOOL;
    qu_out : BOOL;
    qd_out : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := cu_in, CD := cd_in, R := reset, LD := load, PV := 3,
          QU => qu_out, QD => qd_out, CV => cv_out);
END_PROGRAM
";

const CTUD_DINT_PROGRAM: &str = "
PROGRAM main
  VAR
    counter : CTUD_DINT;
    qu_out : BOOL;
    cv_out : DINT;
  END_VAR
  counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 1, QU => qu_out, CV => cv_out);
END_PROGRAM
";

#[rstest]
// CV=0, PV=3: QU = (0 >= 3) FALSE, QD = (0 <= 0) TRUE.
#[case::not_triggered(CTUD_PROGRAM, &[run(0), expect("qu_out", 0), expect("qd_out", 1)])]
// Three up-counts reach PV: CV=3, QU TRUE.
#[case::counts_up(CTUD_PROGRAM, &[
    pulse("cu_in", 3, 0),
    expect("cv_out", 3), expect("qu_out", 1),
])]
// One down-count from 0: CV=-1, QD TRUE.
#[case::counts_down(CTUD_PROGRAM, &[
    write("cd_in", 1), run(0), expect("cv_out", -1), expect("qd_out", 1),
])]
// Reset zeroes CV after two up-counts.
#[case::reset(CTUD_PROGRAM, &[
    pulse("cu_in", 2, 0), expect("cv_out", 2),
    write("reset", 1), run(4), expect("cv_out", 0),
])]
// Load sets CV=PV=3.
#[case::load(CTUD_PROGRAM, &[write("load", 1), run(0), expect("cv_out", 3)])]
// CTUD_DINT variant compiles and runs; one up-count reaches PV=1.
#[case::dint_variant(CTUD_DINT_PROGRAM, &[run(0), expect("qu_out", 1), expect("cv_out", 1)])]
fn end_to_end_fb_ctud(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
