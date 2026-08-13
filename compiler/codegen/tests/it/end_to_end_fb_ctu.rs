//! End-to-end tests for function block invocation (CTU count up counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTU function block instance, compile to bytecode, and execute on the VM.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, FbStep, FbStep::*};

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
#[case::not_triggered(CTU_PROGRAM, &[Run(0), Expect(3, 0)])]
// Three counts reach PV=3: Q TRUE, CV=3.
#[case::counts_to_pv(CTU_PROGRAM, &[
    Pulse { var: 1, n: 3, time_base: 0 },
    Expect(3, 1), Expect(4, 3),
])]
// Reset zeroes CV and clears Q after two counts.
#[case::reset(CTU_PROGRAM, &[
    Pulse { var: 1, n: 2, time_base: 0 }, Expect(4, 2),
    Write(2, 1), Run(4), Expect(4, 0), Expect(3, 0),
])]
// CV=2 < PV=3: Q FALSE.
#[case::below_pv(CTU_PROGRAM, &[
    Pulse { var: 1, n: 2, time_base: 0 }, Expect(3, 0),
])]
// CTU_DINT variant compiles and runs; one count reaches PV=1.
#[case::dint_variant(CTU_DINT_PROGRAM, &[Run(0), Expect(1, 1), Expect(2, 1)])]
fn end_to_end_fb_ctu(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
