//! End-to-end tests for function block invocation (CTU count up counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTU function block instance, compile to bytecode, and execute on the VM.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, expect, pulse, run, write, FbStep};

const CTU_PROGRAM: &str = "
PROGRAM main
  VAR
    counter : CTU;
    pulse : BOOL;
    reset : BOOL;
    result : BOOL;
    count : INT;
  END_VAR
  counter(CU := pulse, R := reset, PV := 3, Q => result, CV => count);
END_PROGRAM
";

const CTU_DINT_PROGRAM: &str = "
PROGRAM main
  VAR
    counter : CTU_DINT;
    result : BOOL;
    count : DINT;
  END_VAR
  counter(CU := TRUE, R := FALSE, PV := 1, Q => result, CV => count);
END_PROGRAM
";

#[rstest]
// CU never pulses: Q stays FALSE.
#[case::not_triggered(CTU_PROGRAM, &[run(0), expect("result", 0)])]
// Three counts reach PV=3: Q TRUE, CV=3.
#[case::counts_to_pv(CTU_PROGRAM, &[
    pulse("pulse", 3, 0),
    expect("result", 1), expect("count", 3),
])]
// Reset zeroes CV and clears Q after two counts.
#[case::reset(CTU_PROGRAM, &[
    pulse("pulse", 2, 0), expect("count", 2),
    write("reset", 1), run(4), expect("count", 0), expect("result", 0),
])]
// CV=2 < PV=3: Q FALSE.
#[case::below_pv(CTU_PROGRAM, &[
    pulse("pulse", 2, 0), expect("result", 0),
])]
// CTU_DINT variant compiles and runs; one count reaches PV=1.
#[case::dint_variant(CTU_DINT_PROGRAM, &[run(0), expect("result", 1), expect("count", 1)])]
fn end_to_end_fb_ctu(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
