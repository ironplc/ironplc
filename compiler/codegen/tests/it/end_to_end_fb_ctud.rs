//! End-to-end tests for function block invocation (CTUD count up/down counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTUD function block instance, compile to bytecode, and execute on the VM.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, FbStep, FbStep::*};

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
#[case::not_triggered(CTUD_PROGRAM, &[Run(0), Expect(5, 0), Expect(6, 1)])]
// Three up-counts reach PV: CV=3, QU TRUE.
#[case::counts_up(CTUD_PROGRAM, &[
    Pulse { var: 1, n: 3, time_base: 0 },
    Expect(7, 3), Expect(5, 1),
])]
// One down-count from 0: CV=-1, QD TRUE.
#[case::counts_down(CTUD_PROGRAM, &[
    Write(2, 1), Run(0), Expect(7, -1), Expect(6, 1),
])]
// Reset zeroes CV after two up-counts.
#[case::reset(CTUD_PROGRAM, &[
    Pulse { var: 1, n: 2, time_base: 0 }, Expect(7, 2),
    Write(3, 1), Run(4), Expect(7, 0),
])]
// Load sets CV=PV=3.
#[case::load(CTUD_PROGRAM, &[Write(4, 1), Run(0), Expect(7, 3)])]
// CTUD_DINT variant compiles and runs; one up-count reaches PV=1.
#[case::dint_variant(CTUD_DINT_PROGRAM, &[Run(0), Expect(1, 1), Expect(2, 1)])]
fn end_to_end_fb_ctud(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
